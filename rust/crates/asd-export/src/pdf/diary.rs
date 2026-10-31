//! owner: WP-B.8 (moved from `asd-report` into `asd-export` by WP-I.1, XC-2)
//!
//! The shareable observation diary PDF: a port of the iOS `DoctorReportGenerator`.
//!
//! Same page (US Letter, 36 pt margins), same sections in the same order and positions,
//! same copy: header, child details, privacy seal, gaze observations, per-clip table,
//! M-CHAT-R/F, home routine, visit notes, sign-off and the mandatory non-diagnostic
//! sentence and mission line (CLM-5). It reports measurements only; an unmeasured metric
//! reads "Not observed" (PLC-2).
//!
//! Differences from iOS, all deliberate:
//! - simulated and synthetic sessions are refused ([`ReportError::Refused`]) instead of
//!   drawing a "SIMULATED DATA" banner: a server report is a record, not a preview;
//! - an unknown age or home-routine figure reads "Not recorded" (iOS defaults the
//!   completion rate to 100% and the age to 24 months);
//! - Helvetica replaces the system font, so line widths differ slightly;
//! - the output is deterministic: no creation date, no document id.

use asd_contract::session_v1_6::GazeSessionRecord;
use asd_core::types::CaptureSource;

use super::canvas::{colors, Canvas, Font, PAGE_HEIGHT, PAGE_WIDTH};
use crate::claims;
use crate::mchat::{MchatResult, MchatRiskLevel};
use crate::{export_refusal, ids, ReportError};

/// Media type of the rendered report.
pub const CONTENT_TYPE: &str = "application/pdf";
/// `DoctorReportInput.guardianName` default.
pub const DEFAULT_GUARDIAN: &str = "Parent / Legal Guardian";
/// Shown for a value nobody recorded (never a made-up default).
pub const NOT_RECORDED: &str = "Not recorded";
/// Shown for a metric the session did not measure (PLC-2).
pub const NOT_OBSERVED: &str = "Not observed";

const MARGIN: f32 = 36.0;
const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
const DOCUMENT_TITLE: &str = "Social Attention Observation Diary";

/// Home routine figures for the last 7 days (CareKit on iOS); `None` = not recorded.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CareSummary {
    pub completion_rate_percent: Option<f64>,
    pub medications_logged: Option<i64>,
    pub diary_entries_logged: Option<i64>,
}

/// Everything the report shows.
pub struct DoctorReportInput<'a> {
    /// The "CHILD" cell: the participant token (the server never has a name, CNS-7).
    pub participant_label: String,
    pub child_age_months: Option<i64>,
    pub guardian_name: String,
    /// "OBSERVATION DATE", RFC 3339; shown as a UTC calendar date.
    pub assessment_date: String,
    pub gaze_record: Option<&'a GazeSessionRecord>,
    pub mchat: Option<&'a MchatResult>,
    pub care: Option<CareSummary>,
}

/// Renders the report. Refuses simulated and synthetic sessions (SIM-5).
pub fn render(input: &DoctorReportInput<'_>) -> Result<Vec<u8>, ReportError> {
    if let Some(record) = input.gaze_record {
        if record.capture_source != CaptureSource::TrueDepth {
            if let Some(refusal) = export_refusal(record) {
                return Err(ReportError::Refused(refusal));
            }
        }
    }
    let date = ids::parse("assessmentDate", &input.assessment_date)?
        .format("%b %-d, %Y")
        .to_string();

    let mut c = Canvas::new();
    let mut y = MARGIN;
    y = header(&mut c, y);
    y += 12.0;
    y = demographics(&mut c, input, &date, y);
    y += 14.0;
    y = privacy_seal(&mut c, y);
    y += 14.0;
    y = gaze_section(&mut c, input.gaze_record, y);
    y += 14.0;
    y = per_video_section(&mut c, input.gaze_record, y);
    y += 14.0;
    y = mchat_section(&mut c, input.mchat, y);
    y += 14.0;
    y = care_section(&mut c, input.care.unwrap_or_default(), y);
    y += 14.0;
    visit_notes(&mut c, input.mchat, y);
    footer(&mut c, PAGE_HEIGHT - MARGIN - 48.0);
    Ok(c.finish(DOCUMENT_TITLE))
}

/// Swift `Int(x)`: truncation toward zero.
fn trunc(x: f64) -> i64 {
    x as i64
}

fn percent(value: Option<f64>) -> String {
    value.map_or_else(
        || NOT_OBSERVED.to_owned(),
        |v| format!("{}%", trunc(v * 100.0)),
    )
}

fn heading(c: &mut Canvas, text: &str, y: f32) {
    c.text(text, MARGIN, y, Font::Bold, 10.0, colors::LABEL);
}

/// Label/value columns inside a box (`label` 8 pt secondary, `value` bold).
fn columns(c: &mut Canvas, items: &[(&str, String)], y: f32, value_size: f32, inset: f32) {
    let col = CONTENT_WIDTH / items.len() as f32;
    for (i, (label, value)) in items.iter().enumerate() {
        let x = MARGIN + inset + i as f32 * col;
        c.text(
            label,
            x,
            y + 8.0,
            Font::Regular,
            8.0,
            colors::SECONDARY_LABEL,
        );
        c.text(value, x, y + 22.0, Font::Bold, value_size, colors::LABEL);
    }
}

fn header(c: &mut Canvas, y: f32) -> f32 {
    c.text(
        "EARLYGAZE OBSERVATION DIARY | PART OF OPEN SPECTRUM TRACKING",
        MARGIN,
        y,
        Font::Bold,
        9.0,
        colors::SECONDARY_LABEL,
    );
    let y = y + 14.0;
    c.text(
        "Social Attention Observation Diary: Summary for Your Pediatrician",
        MARGIN,
        y,
        Font::Bold,
        17.0,
        colors::LABEL,
    );
    let y = y + 24.0;
    c.hline(MARGIN, PAGE_WIDTH - MARGIN, y, 1.0, colors::SEPARATOR);
    y + 4.0
}

fn demographics(c: &mut Canvas, input: &DoctorReportInput<'_>, date: &str, y: f32) -> f32 {
    c.rounded_rect(
        MARGIN,
        y,
        CONTENT_WIDTH,
        46.0,
        6.0,
        colors::SECONDARY_BACKGROUND,
    );
    let age = input
        .child_age_months
        .map_or_else(|| NOT_RECORDED.to_owned(), |m| format!("{m} Months"));
    let items = [
        ("CHILD", input.participant_label.clone()),
        ("AGE", age),
        ("PARENT / GUARDIAN", input.guardian_name.clone()),
        ("OBSERVATION DATE", date.to_owned()),
    ];
    let col = CONTENT_WIDTH / 4.0;
    for (i, (label, value)) in items.iter().enumerate() {
        let x = MARGIN + 10.0 + i as f32 * col;
        c.text(
            label,
            x,
            y + 8.0,
            Font::Regular,
            8.5,
            colors::SECONDARY_LABEL,
        );
        c.text(value, x, y + 22.0, Font::Bold, 9.5, colors::LABEL);
    }
    y + 46.0
}

fn privacy_seal(c: &mut Canvas, y: f32) -> f32 {
    c.rounded_rect(MARGIN, y, CONTENT_WIDTH, 34.0, 6.0, colors::GREEN_TINT);
    c.text(
        "PRIVATE BY DESIGN: NO VIDEO WAS RECORDED",
        MARGIN + 10.0,
        y + 6.0,
        Font::Bold,
        8.5,
        colors::GREEN,
    );
    c.text(
        "No video, image, or audio was stored. Gaze measurements were computed on the device while the child watched.",
        MARGIN + 10.0,
        y + 18.0,
        Font::Regular,
        8.0,
        colors::LABEL,
    );
    y + 34.0
}

fn gaze_section(c: &mut Canvas, record: Option<&GazeSessionRecord>, y: f32) -> f32 {
    heading(c, "1. Gaze Observations: Social vs. Geometric Looking", y);
    let top = y + 18.0;
    c.rounded_rect(
        MARGIN,
        top,
        CONTENT_WIDTH,
        68.0,
        6.0,
        colors::SECONDARY_BACKGROUND,
    );
    let Some(rec) = record else {
        c.text(
            "No gaze observation has been recorded yet.",
            MARGIN + 12.0,
            top + 26.0,
            Font::Regular,
            9.5,
            colors::SECONDARY_LABEL,
        );
        return top + 68.0;
    };
    let pref = trunc(rec.social_preference_score * 100.0);
    let quality = serde_json::to_value(rec.quality_verdict)
        .ok()
        .and_then(|v| v.as_str().map(str::to_uppercase))
        .unwrap_or_default();
    let items = [
        (
            "SOCIAL PREFERENCE",
            format!("{}{pref}%", if pref > 0 { "+" } else { "" }),
        ),
        ("RECORDING QUALITY", quality),
        ("EYE LOOKING SHARE", percent(rec.eye_dwell_ratio)),
        ("MOUTH LOOKING SHARE", percent(rec.mouth_dwell_ratio)),
    ];
    columns(c, &items, top, 13.0, 12.0);
    let latency = rec
        .first_eye_latency_ms
        .map_or_else(|| NOT_OBSERVED.to_owned(), |ms| format!("{} ms", trunc(ms)));
    let sub = format!(
        "First look at eyes: {latency} | Usable gaze samples: {}% | Smile reactivity: {}%",
        trunc(rec.valid_frame_ratio * 100.0),
        trunc(rec.smile_reactivity_index * 100.0)
    );
    c.text(
        &sub,
        MARGIN + 12.0,
        top + 48.0,
        Font::Regular,
        8.0,
        colors::SECONDARY_LABEL,
    );
    top + 68.0
}

/// Per-clip expected-vs-recorded table (v1.5+ records); a compact box otherwise.
fn per_video_section(c: &mut Canvas, record: Option<&GazeSessionRecord>, y: f32) -> f32 {
    heading(c, "2. Per-Clip Engagement (Expected vs Recorded)", y);
    let mut y = y + 18.0;
    let videos = record
        .and_then(|r| r.per_video.as_deref())
        .filter(|v| v.iter().any(|clip| clip.has_baseline));
    let (Some(rec), Some(videos)) = (record, videos) else {
        c.rounded_rect(
            MARGIN,
            y,
            CONTENT_WIDTH,
            40.0,
            6.0,
            colors::SECONDARY_BACKGROUND,
        );
        c.text(
            "Per-clip breakdown unavailable for this session (older record or single-clip capture).",
            MARGIN + 12.0,
            y + 16.0,
            Font::Regular,
            9.5,
            colors::SECONDARY_LABEL,
        );
        return y + 40.0;
    };
    let dim = |c: &mut Canvas, text: &str, x: f32, y: f32| {
        c.text(text, x, y, Font::Regular, 8.0, colors::SECONDARY_LABEL);
    };

    if let Some(composite) = rec.per_video_composite {
        dim(c, "OVERALL ENGAGEMENT", MARGIN + 4.0, y);
        let pct = (composite * 100.0).round() as i64;
        let x = MARGIN + CONTENT_WIDTH - 60.0;
        c.text(
            &format!("{pct}/100"),
            x,
            y - 3.0,
            Font::Bold,
            12.0,
            colors::LABEL,
        );
        y += 16.0;
    }

    for video in videos.iter().filter(|v| v.has_baseline) {
        let pct = (video.subscore * 100.0).round() as i64;
        let status = if video.subscore < 0.35 {
            "Outside age range"
        } else if video.subscore < 0.65 {
            "Near range edge"
        } else {
            "Within age range"
        };
        c.text(
            &video.title,
            MARGIN + 4.0,
            y,
            Font::Regular,
            9.0,
            colors::LABEL,
        );
        let right = MARGIN + CONTENT_WIDTH;
        c.text(
            &format!("{pct}/100"),
            right - 210.0,
            y,
            Font::Bold,
            9.0,
            colors::LABEL,
        );
        c.text(status, right - 160.0, y, Font::Bold, 9.0, colors::LABEL);
        if video.low_confidence {
            dim(c, "low quality", right - 60.0, y);
        }
        y += 13.0;

        for row in &video.rows {
            let name = row.biomarker.replace('_', " ");
            // iOS tests only `contains("latency")` and prints `mean_fixation_duration_ms`
            // as a percentage; every `_ms` biomarker is a duration here.
            let observed = if row.biomarker.contains("latency") || row.biomarker.ends_with("_ms") {
                format!("{} ms", trunc(row.observed))
            } else if row.biomarker == "fixation_count" {
                format!("{:.0}", row.observed)
            } else {
                format!("{}%", trunc(row.observed * 100.0))
            };
            let suffix = if row.imputed {
                " (never looked at)"
            } else {
                ""
            };
            dim(
                c,
                &format!("• {name}: {observed}{suffix}"),
                MARGIN + 14.0,
                y,
            );
            y += 11.0;
        }
        y += 4.0;
    }
    dim(c, claims::AGE_REFERENCE_NOTE, MARGIN + 4.0, y);
    y + 14.0
}

fn mchat_section(c: &mut Canvas, result: Option<&MchatResult>, y: f32) -> f32 {
    heading(c, "3. M-CHAT-R/F Parent Questionnaire", y);
    let y = y + 18.0;
    c.rounded_rect(
        MARGIN,
        y,
        CONTENT_WIDTH,
        60.0,
        6.0,
        colors::SECONDARY_BACKGROUND,
    );
    let Some(m) = result else {
        c.text(
            "M-CHAT-R/F questionnaire pending parent completion.",
            MARGIN + 12.0,
            y + 22.0,
            Font::Regular,
            9.5,
            colors::SECONDARY_LABEL,
        );
        return y + 60.0;
    };
    c.text(
        "QUESTIONNAIRE RESULT",
        MARGIN + 12.0,
        y + 8.0,
        Font::Regular,
        8.0,
        colors::SECONDARY_LABEL,
    );
    c.text(
        claims::mchat_line(m.risk_level),
        MARGIN + 12.0,
        y + 20.0,
        Font::Bold,
        10.0,
        colors::LABEL,
    );
    let flagged = if m.failed_question_ids.is_empty() {
        format!(
            "{} of 20 items flagged. No items to follow up.",
            m.total_failed
        )
    } else {
        let ids: Vec<String> = m
            .failed_question_ids
            .iter()
            .map(|q| format!("#{q}"))
            .collect();
        format!("{} of 20 items flagged: {}", m.total_failed, ids.join(", "))
    };
    c.text(
        &flagged,
        MARGIN + 12.0,
        y + 42.0,
        Font::Regular,
        8.0,
        colors::SECONDARY_LABEL,
    );
    y + 60.0
}

fn care_section(c: &mut Canvas, care: CareSummary, y: f32) -> f32 {
    heading(c, "4. Home Routine and Diary (Last 7 Days)", y);
    let y = y + 18.0;
    c.rounded_rect(
        MARGIN,
        y,
        CONTENT_WIDTH,
        48.0,
        6.0,
        colors::SECONDARY_BACKGROUND,
    );
    let or_unknown = |v: Option<String>| v.unwrap_or_else(|| NOT_RECORDED.to_owned());
    let items = [
        (
            "7-DAY ROUTINE COMPLETION",
            or_unknown(
                care.completion_rate_percent
                    .map(|r| format!("{}%", trunc(r))),
            ),
        ),
        (
            "MEDICATIONS LOGGED",
            or_unknown(care.medications_logged.map(|n| format!("{n} Doses"))),
        ),
        (
            "DIARY ENTRIES RECORDED",
            or_unknown(care.diary_entries_logged.map(|n| format!("{n} Entries"))),
        ),
    ];
    columns(c, &items, y, 12.0, 12.0);
    y + 48.0
}

fn visit_notes(c: &mut Canvas, mchat: Option<&MchatResult>, y: f32) {
    heading(c, "5. Notes for the Visit", y);
    let y = y + 18.0;
    let mut text = String::from(
        "This diary records what the child looked at and how the family routine went. \
         Please share it at your next pediatric visit. Only a qualified professional can \
         evaluate a child's development.",
    );
    if mchat.is_some_and(|m| m.risk_level != MchatRiskLevel::Low) {
        text.push_str(
            " The M-CHAT-R/F result above has a follow-up step: please discuss it with your pediatrician.",
        );
    }
    c.rounded_rect(MARGIN, y, CONTENT_WIDTH, 50.0, 6.0, colors::BLUE_TINT);
    c.text_in_rect(
        &text,
        MARGIN + 10.0,
        y + 8.0,
        CONTENT_WIDTH - 20.0,
        36.0,
        Font::Regular,
        8.5,
        colors::LABEL,
    );
}

fn footer(c: &mut Canvas, y: f32) {
    c.hline(MARGIN, PAGE_WIDTH - MARGIN, y, 0.5, colors::SEPARATOR);
    c.text(
        "Reviewed by: ___________________________    Date: _________________",
        MARGIN,
        y + 8.0,
        Font::Regular,
        8.0,
        colors::SECONDARY_LABEL,
    );
    // Mandatory sentences (CLM-5), each its own text run so they extract verbatim.
    c.text(
        claims::NOT_DIAGNOSTIC,
        MARGIN,
        y + 22.0,
        Font::Bold,
        8.5,
        colors::LABEL,
    );
    c.text(
        claims::MISSION,
        MARGIN,
        y + 34.0,
        Font::Regular,
        8.0,
        colors::SECONDARY_LABEL,
    );
}

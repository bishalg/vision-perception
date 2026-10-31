//! Observation diary PDF parity with the iOS `DoctorReportGenerator` (CLM-3..5, PLC-2,
//! SIM-5) and byte-for-byte golden output.
//!
//! The iOS generator has no committed golden PDF (UIKit output is not reproducible across
//! OS versions), so the parity oracle is its copy and section structure, asserted line by
//! line in `doctor-report.txt`; `doctor-report.pdf` pins the exact bytes of this renderer.

mod support;

use asd_export::mchat::{MchatResult, MchatRiskLevel};
use asd_export::pdf::{self, CareSummary, DoctorReportInput};
use asd_export::{claims, Refusal, ReportError};
use serde_json::json;
use support::*;

fn golden_input(record: &asd_contract::session_v1_6::GazeSessionRecord) -> DoctorReportInput<'_> {
    DoctorReportInput {
        participant_label: record.participant_id.clone(),
        child_age_months: Some(28),
        guardian_name: pdf::DEFAULT_GUARDIAN.into(),
        assessment_date: record.started_at.clone(),
        gaze_record: Some(record),
        mchat: None,
        care: None,
    }
}

fn high_mchat() -> MchatResult {
    MchatResult {
        total_failed: 9,
        risk_level: MchatRiskLevel::High,
        failed_question_ids: vec![1, 5, 7],
        completion_date: "2026-10-01T09:00:00Z".into(),
    }
}

fn text(input: &DoctorReportInput<'_>) -> String {
    pdf_text_runs(&pdf::render(input).expect("render")).join("\n")
}

#[test]
fn report_matches_golden_bytes_and_text() {
    let record = ios_session_record();
    let bytes = pdf::render(&golden_input(&record)).unwrap();
    let mut lines = pdf_text_runs(&bytes).join("\n");
    lines.push('\n');
    assert_golden_bytes("doctor-report.txt", lines.as_bytes());
    assert_golden_bytes("doctor-report.pdf", &bytes);
}

#[test]
fn rendering_is_deterministic() {
    let record = ios_session_record();
    let a = pdf::render(&golden_input(&record)).unwrap();
    let b = pdf::render(&golden_input(&record)).unwrap();
    assert_eq!(a, b);
    assert!(a.starts_with(b"%PDF-"));
    // No creation date or document id that would change between runs.
    let s = String::from_utf8_lossy(&a);
    assert!(!s.contains("/CreationDate") && !s.contains("/ID"));
}

#[test]
fn report_is_one_us_letter_page() {
    let record = ios_session_record();
    let bytes = pdf::render(&golden_input(&record)).unwrap();
    assert_eq!(pdf_page_count(&bytes), 1);
    assert!(String::from_utf8_lossy(&bytes).contains("/MediaBox [0 0 612 792]"));
}

#[test]
fn both_mandatory_sentences_are_present_with_minimal_data() {
    let input = DoctorReportInput {
        participant_label: "MIN-1".into(),
        child_age_months: None,
        guardian_name: pdf::DEFAULT_GUARDIAN.into(),
        assessment_date: "2026-10-01T10:00:00Z".into(),
        gaze_record: None,
        mchat: None,
        care: None,
    };
    let t = text(&input);
    assert!(t.contains(claims::NOT_DIAGNOSTIC), "{t}");
    assert!(t.contains(claims::MISSION), "{t}");
    assert!(t.contains("No gaze observation has been recorded yet."));
    assert!(t.contains("M-CHAT-R/F questionnaire pending parent completion."));
    assert_eq!(claims::NOT_DIAGNOSTIC, "This is not a diagnostic tool.");
    assert_eq!(claims::MISSION, "Part of Open Spectrum Tracking");
}

#[test]
fn high_mchat_names_the_instrument_and_never_uses_forbidden_wording() {
    let record = ios_session_record();
    let mchat = high_mchat();
    let mut input = golden_input(&record);
    input.mchat = Some(&mchat);
    let t = text(&input);
    assert!(t.contains("M-CHAT-R/F: high (follow-up needed; discuss with your pediatrician soon)"));
    assert!(t.contains("9 of 20 items flagged: #1, #5, #7"));
    // The visit note wraps over several text runs.
    assert!(t
        .replace('\n', " ")
        .contains("has a follow-up step: please discuss it with your pediatrician."));
    let lower = t.to_lowercase();
    for forbidden in [
        "elevated risk",
        "high risk",
        "risk stratification",
        "confidence",
        "irb",
        "fda",
        "atypical",
        "screening",
        "diagnosed",
    ] {
        assert!(
            !lower.contains(forbidden),
            "PDF must not contain {forbidden}"
        );
    }
}

#[test]
fn mchat_line_always_names_instrument_and_next_step() {
    for level in [
        MchatRiskLevel::Low,
        MchatRiskLevel::Medium,
        MchatRiskLevel::High,
    ] {
        let line = claims::mchat_line(level);
        assert!(line.starts_with("M-CHAT-R/F:"), "{line}");
        assert!(line.contains("pediatrician"), "{line}");
    }
}

#[test]
fn absent_metrics_render_as_not_observed_never_zero() {
    let record = record_with(
        &ios_session_record(),
        json!({"eyeDwellRatio": null, "mouthDwellRatio": null, "firstEyeLatencyMs": null}),
    );
    let t = text(&golden_input(&record));
    assert!(t.contains("Not observed"));
    assert!(t.contains("First look at eyes: Not observed"));
    assert!(!t.contains("EYE LOOKING SHARE\n0%"));
}

#[test]
fn per_clip_rows_follow_the_ios_wording() {
    let record = ios_session_record();
    let t = text(&golden_input(&record));
    assert!(t.contains("OVERALL ENGAGEMENT"));
    // Imputed latency row: shown with the "never looked at" suffix.
    assert!(t.contains("(never looked at)"), "{t}");
    assert!(t.contains("fixation count: 14"), "{t}");
    // Durations in ms are shown in ms. iOS only checks for "latency" and prints this row as
    // "28000%" (a known iOS bug, not ported).
    assert!(t.contains("mean fixation duration ms: 280 ms"), "{t}");
    assert!(t.contains("Age references are provisional"));
    assert!(t.contains("They are not a diagnosis."));
}

#[test]
fn unknown_care_and_age_are_not_invented() {
    let record = ios_session_record();
    let mut input = golden_input(&record);
    input.child_age_months = None;
    let t = text(&input);
    // iOS defaults an unknown completion rate to 100%; the server never invents a value.
    assert!(!t.contains("100%"), "{t}");
    assert!(t.contains("Not recorded"));

    input.care = Some(CareSummary {
        completion_rate_percent: Some(92.0),
        medications_logged: Some(14),
        diary_entries_logged: Some(7),
    });
    let t = text(&input);
    assert!(t.contains("92%") && t.contains("14 Doses") && t.contains("7 Entries"));
}

#[test]
fn synthetic_and_simulated_sessions_are_refused() {
    for (source, refusal) in [
        ("synthetic", Refusal::SyntheticCapture),
        ("simulated", Refusal::SimulatedCapture),
    ] {
        let record = record_with(&ios_session_record(), json!({"captureSource": source}));
        let err = pdf::render(&golden_input(&record)).unwrap_err();
        assert!(
            matches!(err, ReportError::Refused(r) if r == refusal),
            "{source}: {err}"
        );
    }
}

#[test]
fn text_outside_winansi_is_replaced_not_dropped() {
    let record = ios_session_record();
    let mut input = golden_input(&record);
    input.participant_label = "child-\u{0928}\u{0947} (Zoë)".into();
    let t = text(&input);
    assert!(t.contains("child-?? (Zoë)"), "{t}");
}

#[test]
fn observation_date_is_rendered_in_utc_medium_style() {
    let record = ios_session_record();
    let t = text(&golden_input(&record));
    assert!(t.contains("Oct 26, 2026"), "{t}");
}

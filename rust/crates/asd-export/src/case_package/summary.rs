//! owner: WP-I.12
//!
//! `report/case-summary.pdf` (SHR-5), feature `pdf`: a cover (org, mode, purpose, export
//! id, recipient, participant, consent in identified mode), one section per session in
//! date order starting on its own page (session facts, measurements, the per-clip table,
//! the clinician review with a `PRELIMINARY - NOT SIGNED` banner when unsigned), and the
//! encounter list. Every page carries the footer of PDF-4 (watermark line, page n/m) and
//! both mandatory sentences (PDF-5, XC-6).
//!
//! Drawn with the [`crate::pdf::canvas`] primitives the diary uses (base-14 Helvetica,
//! WinAnsi, deterministic bytes). The single-session clinical summary of WP-I.5 is still a
//! stub; when it lands with PDF/A-2b and Devanagari fonts this renderer moves onto it
//! without changing the package contract. De-identified packages render the de-identified
//! records, so free text, signer names and exact dates never reach the page.

use chrono::{DateTime, Utc};

use super::Case;
use crate::claims;
use crate::model::{Disposition, ExportRecord, OrgFacts, ReviewState, SignatureRole};
use crate::pdf::canvas::{colors, finish_pages, wrap, Canvas, Font, Rgb, PAGE_HEIGHT, PAGE_WIDTH};
use crate::watermark::MANDATORY_SENTENCE;
use crate::ExportError;

const MARGIN: f32 = 48.0;
const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
/// Body content stays above this line; the footer is drawn below it.
const FOOTER_TOP: f32 = PAGE_HEIGHT - 84.0;
const BODY: f32 = 9.5;
const LEADING: f32 = 1.35;
const LABEL_WIDTH: f32 = 150.0;
const TITLE: &str = "Case summary";
/// Not measured: never a zero (PLC-2).
const NOT_OBSERVED: &str = "Not observed";
const BANNER_TINT: Rgb = Rgb(1.0, 0.953, 0.878);
const BANNER_TEXT: Rgb = Rgb(0.6, 0.302, 0.0);

/// Banner of an unsigned session (PDF-4).
pub(crate) const PRELIMINARY_BANNER: &str = "PRELIMINARY - NOT SIGNED";
const AWAITING_COSIGN_BANNER: &str = "AWAITING SECOND SIGNATURE";
const AMENDING_BANNER: &str = "PRELIMINARY - AMENDMENT IN PROGRESS";

struct Doc {
    pages: Vec<Canvas>,
    y: f32,
}

impl Doc {
    fn new() -> Self {
        let mut d = Self {
            pages: Vec::new(),
            y: MARGIN,
        };
        d.new_page();
        d
    }

    fn new_page(&mut self) {
        self.pages.push(Canvas::new());
        self.y = MARGIN;
    }

    fn canvas(&mut self) -> &mut Canvas {
        self.pages.last_mut().expect("at least one page")
    }

    fn ensure(&mut self, height: f32) {
        if self.y + height > FOOTER_TOP {
            self.new_page();
        }
    }

    fn gap(&mut self, h: f32) {
        self.y += h;
    }

    fn heading(&mut self, text: &str, size: f32) {
        self.ensure(size * 2.4);
        let y = self.y;
        self.canvas()
            .text(text, MARGIN, y, Font::Bold, size, colors::LABEL);
        self.y += size * 1.5;
    }

    fn paragraph(&mut self, text: &str, font: Font, size: f32, color: Rgb) {
        for line in wrap(text, CONTENT_WIDTH, font, size) {
            self.ensure(size * LEADING);
            let y = self.y;
            self.canvas().text(&line, MARGIN, y, font, size, color);
            self.y += size * LEADING;
        }
    }

    /// `label: value`, the value wrapped in its column.
    fn field(&mut self, label: &str, value: &str) {
        let lines = wrap(value, CONTENT_WIDTH - LABEL_WIDTH, Font::Regular, BODY);
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        self.ensure(BODY * LEADING);
        let y = self.y;
        self.canvas().text(
            label,
            MARGIN,
            y,
            Font::Regular,
            BODY,
            colors::SECONDARY_LABEL,
        );
        for line in lines {
            self.ensure(BODY * LEADING);
            let y = self.y;
            self.canvas().text(
                &line,
                MARGIN + LABEL_WIDTH,
                y,
                Font::Regular,
                BODY,
                colors::LABEL,
            );
            self.y += BODY * LEADING;
        }
    }

    /// One table row; `cells` are `(x offset, text)`.
    fn row(&mut self, cells: &[(f32, String)], font: Font, size: f32) {
        self.ensure(size * LEADING);
        let y = self.y;
        for (x, text) in cells {
            self.canvas()
                .text(text, MARGIN + x, y, font, size, colors::LABEL);
        }
        self.y += size * LEADING;
    }

    fn banner(&mut self, text: &str) {
        self.ensure(30.0);
        let y = self.y;
        let c = self.canvas();
        c.rounded_rect(MARGIN, y, CONTENT_WIDTH, 22.0, 4.0, BANNER_TINT);
        c.text(text, MARGIN + 10.0, y + 6.0, Font::Bold, 10.0, BANNER_TEXT);
        self.y += 30.0;
    }

    fn rule(&mut self) {
        self.ensure(8.0);
        let y = self.y + 3.0;
        self.canvas()
            .hline(MARGIN, PAGE_WIDTH - MARGIN, y, 0.5, colors::SEPARATOR);
        self.y += 8.0;
    }
}

/// Footer of every page: both mandatory sentences, the watermark line (shrunk to fit,
/// never wrapped, so it stays one text run) and `Page n/m`.
fn footers(pages: &mut [Canvas], watermark: &str) {
    let total = pages.len();
    for (i, c) in pages.iter_mut().enumerate() {
        let top = FOOTER_TOP + 8.0;
        c.hline(MARGIN, PAGE_WIDTH - MARGIN, top, 0.5, colors::SEPARATOR);
        c.text(
            MANDATORY_SENTENCE,
            MARGIN,
            top + 8.0,
            Font::Bold,
            8.0,
            colors::LABEL,
        );
        c.text(
            claims::MISSION,
            MARGIN,
            top + 20.0,
            Font::Regular,
            8.0,
            colors::SECONDARY_LABEL,
        );
        let mut size = 7.0;
        while size > 4.0 && Font::Regular.text_width(watermark, size) > CONTENT_WIDTH {
            size -= 0.5;
        }
        c.text(
            watermark,
            MARGIN,
            top + 32.0,
            Font::Regular,
            size,
            colors::SECONDARY_LABEL,
        );
        let page = format!("Page {}/{}", i + 1, total);
        let w = Font::Regular.text_width(&page, 7.0);
        c.text(
            &page,
            PAGE_WIDTH - MARGIN - w,
            top + 44.0,
            Font::Regular,
            7.0,
            colors::SECONDARY_LABEL,
        );
    }
}

fn parse(field: &'static str, t: &str) -> Result<DateTime<Utc>, ExportError> {
    DateTime::parse_from_rfc3339(t)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| ExportError::InvalidTimestamp { field })
}

/// `YYYY-MM-DD HH:MM (+05:45)` in the org zone (PDF-3).
fn local(org: &OrgFacts, field: &'static str, t: &str) -> Result<String, ExportError> {
    let tz = org.tz()?;
    Ok(parse(field, t)?
        .with_timezone(&tz)
        .format("%Y-%m-%d %H:%M (%:z)")
        .to_string())
}

fn num(v: Option<f64>, digits: usize) -> String {
    v.map_or_else(|| NOT_OBSERVED.to_owned(), |x| format!("{x:.digits$}"))
}

fn wire<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s.replace('_', " "),
        _ => String::new(),
    }
}

fn disposition_display(d: Disposition) -> &'static str {
    match d {
        Disposition::NoConcernNoted => "No concern noted",
        Disposition::Monitor => "Monitor",
        Disposition::ReferForEvaluation => "Refer for evaluation",
        Disposition::RecaptureRequested => "Recapture requested",
        Disposition::NotInterpretable => "Not interpretable",
    }
}

/// PDF-4 banner of a session, `None` for a final or amended signed review.
pub(crate) fn banner(record: &ExportRecord) -> Option<&'static str> {
    match record.review.as_ref() {
        None => Some(PRELIMINARY_BANNER),
        Some(r) if !r.is_signed() => Some(PRELIMINARY_BANNER),
        Some(r) if r.state == ReviewState::AwaitingCosign => Some(AWAITING_COSIGN_BANNER),
        Some(r) if r.state == ReviewState::Amending => Some(AMENDING_BANNER),
        Some(r) if r.state != ReviewState::Final => Some(PRELIMINARY_BANNER),
        Some(_) => None,
    }
}

pub(crate) fn render(case: &Case<'_>) -> Result<Vec<u8>, ExportError> {
    let mut d = Doc::new();
    cover(&mut d, case)?;
    let n = case.records.len();
    for (i, record) in case.records.iter().enumerate() {
        d.new_page();
        session(&mut d, case, record, i + 1, n)?;
    }
    if !case.encounters.is_empty() {
        d.new_page();
        encounters(&mut d, case)?;
    }
    let mut pages = d.pages;
    footers(&mut pages, &case.input.watermark.line());
    Ok(finish_pages(pages, TITLE))
}

fn cover(d: &mut Doc, case: &Case<'_>) -> Result<(), ExportError> {
    let input = case.input;
    let wm = &input.watermark;
    let org = &case.org;
    d.heading(TITLE, 20.0);
    d.paragraph(&org.name, Font::Bold, 12.0, colors::LABEL);
    for l in org.letterhead.iter().take(4) {
        d.paragraph(l, Font::Regular, BODY, colors::SECONDARY_LABEL);
    }
    d.rule();
    d.field("Export id", &wm.export_id);
    d.field("Mode", wm.mode.label());
    d.field("Purpose", wm.purpose.label());
    d.field("Recipient", case.recipient_label());
    d.field(
        if case.deidentified() {
            "Participant (pseudonym)"
        } else {
            "Participant token"
        },
        case.participant_label(),
    );
    if let Some(mrn) = case.participant.mrn.as_ref() {
        d.field(
            "Hospital record number",
            &format!("{} ({})", mrn.value, mrn.assigning_authority),
        );
    }
    if let Some(age) = case.participant.age_bracket_at_enrol.as_deref() {
        d.field("Age bracket at enrolment", age);
    }
    d.field("Sessions", &case.records.len().to_string());
    let (from, to) = case.date_range()?;
    d.field("Date range", &format!("{from} to {to}"));
    if !case.deidentified() {
        if let Some(c) = input.consent.as_ref() {
            d.field(
                "Consent",
                &format!(
                    "{}, recorded {} by {}",
                    c.reference,
                    local(org, "consent.recorded_at", &c.recorded_at)?,
                    c.recorded_by_display
                ),
            );
        }
    }
    d.field("Requested by", &wm.requested_by_display);
    d.field("Requested at (UTC)", &wm.requested_at_utc);
    d.gap(10.0);
    d.paragraph(
        "This summary reports measurements from gaze engagement observations and the \
         clinic's review status. A disposition, where shown, is the clinician's statement.",
        Font::Regular,
        BODY,
        colors::LABEL,
    );
    if case.records.is_empty() {
        d.gap(4.0);
        d.paragraph(
            "This case holds no observation session yet: it carries the child's visits              and documents only.",
            Font::Regular,
            BODY,
            colors::LABEL,
        );
    }
    if case.deidentified() {
        d.gap(4.0);
        d.paragraph(
            "De-identified: dates are months, ids are pseudonyms valid inside this package \
             only, and free text and clinician names are left out. Do not attempt to \
             re-identify the child.",
            Font::Regular,
            BODY,
            colors::SECONDARY_LABEL,
        );
    }
    Ok(())
}

fn session(
    d: &mut Doc,
    case: &Case<'_>,
    r: &ExportRecord,
    index: usize,
    total: usize,
) -> Result<(), ExportError> {
    let s = &r.session;
    let sm = &s.summary;
    d.heading(
        &format!("Session {index} of {total} · {}", case.session_date(r)?),
        14.0,
    );
    if let Some(text) = banner(r) {
        d.banner(text);
    }
    if case.deidentified() {
        d.field("Session (pseudonym)", &sm.id);
        let day = r.days_since_first_session.map_or_else(String::new, |n| {
            format!(", day {n} since the first session")
        });
        d.field("Started", &format!("{}{day}", sm.started_at));
    } else {
        d.field("Session id", &sm.id);
        d.field(
            "Started",
            &local(&r.org, "session.started_at", &sm.started_at)?,
        );
    }
    d.field("Duration", &format!("{:.0} s", sm.duration_sec));
    if let Some(c) = sm.capture_source.as_ref() {
        d.field("Capture", &wire(c));
    }
    d.field("Quality verdict", &sm.quality_verdict.replace('_', " "));
    d.field("Valid frames", &num(s.valid_frame_ratio, 2));
    if let Some(pack) = s.age_pack.as_deref() {
        d.field("Age pack", pack);
    }
    if let Some(v) = sm.bundle_version.as_deref() {
        d.field("Content bundle", v);
    }

    d.gap(6.0);
    d.heading("Measurements", 11.0);
    let unscorable = sm
        .scoring_status
        .as_ref()
        .is_some_and(|st| wire(st) == "unscorable");
    if unscorable {
        let reason = sm
            .unscorable_reason
            .as_ref()
            .map_or_else(String::new, |why| format!(" ({})", wire(why)));
        d.field("Composite score", &format!("Not scored{reason}"));
    } else {
        d.field("Composite score", &num(sm.composite_score, 2));
    }
    d.field("Social preference", &num(s.social_preference, 2));
    d.field("Eye dwell", &num(s.eye_dwell, 2));
    d.field("Mouth dwell", &num(s.mouth_dwell, 2));
    d.field("First eye latency (ms)", &num(s.first_eye_latency_ms, 0));

    d.gap(6.0);
    d.heading("Per clip", 11.0);
    if r.clips.is_empty() {
        d.paragraph(
            "No clips recorded.",
            Font::Regular,
            BODY,
            colors::SECONDARY_LABEL,
        );
    } else {
        let cols = [0.0, 150.0, 190.0, 290.0, 350.0, 430.0];
        let header = [
            "Clip",
            "Order",
            "Status",
            "Score",
            "Social dwell",
            "Latency (ms)",
        ];
        d.row(
            &cols
                .iter()
                .zip(header)
                .map(|(x, h)| (*x, h.to_owned()))
                .collect::<Vec<_>>(),
            Font::Bold,
            8.5,
        );
        for k in &r.clips {
            let status = k.status.as_ref().map_or_else(String::new, wire);
            let cells = [
                k.clip_id.clone(),
                k.order.to_string(),
                status,
                num(k.score, 2),
                num(k.dwell_social, 2),
                num(k.orienting_latency_ms, 0),
            ];
            d.row(
                &cols.iter().copied().zip(cells).collect::<Vec<_>>(),
                Font::Regular,
                8.5,
            );
        }
    }

    d.gap(6.0);
    d.heading("Clinician review", 11.0);
    let Some(review) = r.review.as_ref() else {
        d.paragraph(
            "No clinician review recorded.",
            Font::Regular,
            BODY,
            colors::SECONDARY_LABEL,
        );
        return Ok(());
    };
    d.field("Review state", &wire(&review.state));
    d.field("Report status", r.report_status().as_str());
    d.field(
        "Clinician's statement",
        review
            .disposition
            .map_or("No disposition recorded", disposition_display),
    );
    if !case.deidentified() {
        if let Some(t) = review
            .impression
            .as_deref()
            .filter(|t| !t.trim().is_empty())
        {
            d.field("Impression", t);
        }
        if let Some(t) = review
            .recommendations
            .as_deref()
            .filter(|t| !t.trim().is_empty())
        {
            d.field("Recommendations", t);
        }
    }
    for sig in &review.signatures {
        let role = match sig.role {
            SignatureRole::Sign => "Signed",
            SignatureRole::Cosign => "Second signature",
        };
        let text = if case.deidentified() {
            sig.signed_at.clone()
        } else {
            let mut who = sig.display_name.clone().unwrap_or_default();
            if let Some(reg) = sig.professional_registration.as_deref() {
                who = format!("{who} ({reg})");
            }
            format!(
                "{who}, {}",
                local(&r.org, "signature.signed_at", &sig.signed_at)?
            )
        };
        d.field(role, &text);
    }
    if let Some(hash) = review.content_hash.as_deref() {
        d.field("Signed content", &hash.chars().take(12).collect::<String>());
    }
    Ok(())
}

fn encounters(d: &mut Doc, case: &Case<'_>) -> Result<(), ExportError> {
    d.heading("Encounters", 14.0);
    let org = &case.org;
    for e in &case.encounters {
        let when = if case.deidentified() {
            e.occurred_at.clone()
        } else {
            local(org, "encounter.occurred_at", &e.occurred_at)?
        };
        let mut head = format!("{when} · {} · {}", e.kind.label(), wire(&e.status));
        if let Some(who) = e.clinician_display.as_deref() {
            head = format!("{head} · {who}");
        }
        d.gap(4.0);
        d.paragraph(&head, Font::Bold, BODY, colors::LABEL);
        if let Some(reason) = e.reason.as_deref() {
            d.field("Reason", reason);
        }
        if let Some(summary) = e.summary.as_deref() {
            d.field("Summary", summary);
        }
        if let Some(due) = e.follow_up_due_at.as_deref() {
            let due = if case.deidentified() {
                due.to_owned()
            } else {
                local(org, "encounter.follow_up_due_at", due)?
            };
            let done = if e.follow_up_done_at.is_some() {
                " (done)"
            } else {
                ""
            };
            d.field("Follow-up due", &format!("{due}{done}"));
        }
    }
    Ok(())
}

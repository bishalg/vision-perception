//! owner: WP-I.4
//!
//! HL7 v2.5.1 ORU^R01 encoder (HL7-1..7), hand-written: [`escape`] (delimiter escaping,
//! trap 10) and [`segments`] (MSH, PID, PV1, ORC, OBR, OBX, NTE). Identified mode only;
//! refuses a record without an MRN and any de-identified export (an HL7 delivery to a
//! hospital system is a clinical-care artifact; the MRN is its reason to exist).
//!
//! One message per call, segments terminated by CR, no MLLP framing and no FHS/BHS
//! envelope (WP-I.11). Output is deterministic (XC-8): `generated_at` and the delivery id
//! are inputs.

use std::collections::BTreeMap;

use base64::Engine as _;
use sha2::{Digest, Sha256};

use crate::codes::{self, CodePolicy};
use crate::model::{ClinicalReviewSummary, ExportRecord, ReportStatus};
use crate::policy::Mode;
use crate::watermark::Watermark;
use crate::ExportError;

pub mod escape;
pub mod segments;

use segments::{ObrFacts, ObxCode, ObxValue};

/// Why an HL7 message cannot be produced (besides the shared [`ExportError`] causes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Hl7Error {
    /// HL7-3: a delivery without an MRN is refused.
    #[error("HL7 v2 export needs the participant's MRN (HL7-3)")]
    MissingMrn,
    /// HL7 is identified-only.
    #[error("HL7 v2 export is identified-only; de-identified records are refused")]
    DeidentifiedRefused,
    /// The profile sets `embed_pdf` but no PDF bytes were supplied.
    #[error("the profile embeds the PDF summary but none was supplied")]
    PdfMissing,
}

/// `PID-5` policy (HL7-3). No names are stored (MRN-1), so the choice is an empty field or
/// the pseudonymous participant token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pid5Mode {
    #[default]
    Empty,
    Token,
}

/// A hospital code that replaces a local `OBX-3` (profile `code_map`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedCode {
    pub code: String,
    pub text: String,
    /// Coding system name, e.g. `99HOSP` or `LN`.
    pub system: String,
}

/// The integration profile as the encoder needs it (HL7-1..7).
#[derive(Debug, Clone, Default)]
pub struct Hl7Profile {
    pub sending_app: String,
    pub sending_facility: String,
    pub receiving_app: String,
    pub receiving_facility: String,
    /// Add a `PV1` (class `O`).
    pub send_pv1: bool,
    /// `MSH-11 = T` for test destinations.
    pub test_destination: bool,
    pub pid5_mode: Pid5Mode,
    /// Local metric code (`social-preference-score`, ...; see [`METRICS`] and
    /// `disposition`, `impression`, `summary-pdf`) to the hospital's own code.
    pub code_map: BTreeMap<String, MappedCode>,
    /// Add the PDF summary as one `ED` OBX; the bytes go in [`Hl7Options::pdf`].
    pub embed_pdf: bool,
    /// XC-7 opt-in for unverified external codes (the LOINC alternate of `OBR-4`).
    pub code_policy: CodePolicy,
    /// File-name prefix for [`file_name`].
    pub file_prefix: String,
}

/// Per-delivery inputs.
#[derive(Debug, Clone)]
pub struct Hl7Options<'a> {
    pub profile: &'a Hl7Profile,
    /// `MSH-10`.
    pub delivery_id: &'a str,
    /// RFC 3339 (`MSH-7`, `ORC-9`); the crate never reads a clock.
    pub generated_at: &'a str,
    pub watermark: &'a Watermark,
    /// The PDF/A summary to embed when the profile asks for it.
    pub pdf: Option<&'a [u8]>,
}

/// `(local code, display, unit, value)` rows of the gaze metrics in FHIR component order.
/// Public so tests and mappings can enumerate the local codes.
pub const METRICS: [(&str, &str, &str); 9] = [
    (
        "social-preference-score",
        "Social visual preference score",
        "score",
    ),
    ("eye-region-dwell-ratio", "Eye region dwell ratio", "ratio"),
    (
        "mouth-region-dwell-ratio",
        "Mouth region dwell ratio",
        "ratio",
    ),
    ("valid-frame-ratio", "Valid frame ratio", "ratio"),
    (
        "social-smile-intensity-mean",
        "Social smile intensity mean",
        "score",
    ),
    ("smile-reactivity-index", "Smile reactivity index", "score"),
    ("flat-affect-score", "Neutral expression score", "score"),
    ("surprise-score-mean", "Surprise score mean", "score"),
    (
        "composite-engagement",
        "Per-stimulus composite engagement score",
        "score",
    ),
];

/// `affect` map keys of `SessionDetail` for the four affect metrics.
const AFFECT_KEYS: [(&str, &str); 4] = [
    ("social-smile-intensity-mean", "socialSmileIntensity"),
    ("smile-reactivity-index", "smileReactivityIndex"),
    ("flat-affect-score", "flatAffectScore"),
    ("surprise-score-mean", "surpriseScoreMean"),
];

/// The status letter for `OBR-25` and every `OBX-11` (HL7-4).
pub fn status_letter(status: ReportStatus) -> char {
    match status {
        ReportStatus::Preliminary => 'P',
        ReportStatus::Final => 'F',
        ReportStatus::Amended => 'C',
    }
}

/// `{prefix}{export_id}_{session_id_hash}.hl7` (HL7-7). `session_id_hash` is the first 16
/// hex characters of the SHA-256 of the session id; characters outside `[A-Za-z0-9._-]`
/// in the prefix or export id become `_` so the name is safe on any file system.
pub fn file_name(prefix: &str, export_id: &str, session_id: &str) -> String {
    fn safe(s: &str) -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }
    let digest = Sha256::digest(session_id.as_bytes());
    let hash: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("{}{}_{hash}.hl7", safe(prefix), safe(export_id))
}

/// Encode one ORU^R01 message (CR-terminated segments, UTF-8).
pub fn encode_oru_r01(
    record: &ExportRecord,
    options: &Hl7Options<'_>,
) -> Result<Vec<u8>, ExportError> {
    if options.watermark.mode == Mode::Deidentified || record.days_since_first_session.is_some() {
        return Err(Hl7Error::DeidentifiedRefused.into());
    }
    let mrn = record.patient.mrn.as_ref().ok_or(Hl7Error::MissingMrn)?;
    let profile = options.profile;
    let tz = record.org.tz()?;
    let session = &record.session.summary;

    let generated = segments::ts(options.generated_at, tz, "generated_at")?;
    let observed = segments::ts(&session.started_at, tz, "started_at")?;
    let review = record.review.as_ref();
    let signed = review
        .and_then(|r| r.signed_at.as_deref())
        .map(|s| segments::ts(s, tz, "signed_at"))
        .transpose()?;
    let status = record.report_status();
    let letter = status_letter(status);

    // XC-7: the LOINC alternate is added only when the table says it is verified (or the
    // profile opts in). Unverified means "omit the alternate", not "fail the message".
    let alternate = codes::LOINC_GAZE_PANEL
        .require(profile.code_policy)
        .ok()
        .map(|c| (c.code, c.display, "LN"));

    let first_signer = review
        .and_then(|r| r.signatures.first())
        .map(|s| {
            (
                s.user_id.as_deref().unwrap_or(""),
                s.display_name.as_deref(),
            )
        })
        .filter(|(id, name)| !id.is_empty() || name.is_some())
        .map(|(id, name)| (id, name.unwrap_or("")));

    let mut out = String::new();
    out.push_str(&segments::msh(&segments::MshFacts {
        sending_app: &profile.sending_app,
        sending_facility: &profile.sending_facility,
        receiving_app: &profile.receiving_app,
        receiving_facility: &profile.receiving_facility,
        timestamp: &generated,
        control_id: options.delivery_id,
        test: profile.test_destination,
    }));
    let token =
        (profile.pid5_mode == Pid5Mode::Token).then_some(&*record.patient.participant_token);
    out.push_str(&segments::pid(&mrn.value, &mrn.assigning_authority, token));
    if profile.send_pv1 {
        out.push_str(&segments::pv1());
    }
    out.push_str(&segments::orc(
        &session.id,
        &profile.sending_app,
        status != ReportStatus::Preliminary,
        &generated,
    ));
    out.push_str(&segments::obr(&ObrFacts {
        filler: &session.id,
        app: &profile.sending_app,
        alternate,
        observed_ts: &observed,
        signed_ts: signed.as_deref(),
        status: letter,
        first_signer,
    }));
    for (i, line) in options.watermark.lines().iter().enumerate() {
        out.push_str(&segments::nte(i + 1, line));
    }

    let mut set_id = 0usize;
    let mut push = |out: &mut String, code: ObxCode<'_>, value: ObxValue<'_>| {
        set_id += 1;
        out.push_str(&segments::obx(set_id, &code, &value, letter, &observed));
    };

    for (local, display, unit) in METRICS {
        let Some(value) = metric_value(record, local) else {
            continue;
        };
        let code = obx_code(profile, local, display);
        push(&mut out, code, ObxValue::Numeric { value, unit });
    }
    for clip in &record.clips {
        let has_baseline = clip.rows.iter().any(|r| r.expected.is_some());
        let Some(score) = clip.score.filter(|s| s.is_finite() && has_baseline) else {
            continue;
        };
        let local = format!("clip-score-{}", clip.clip_id);
        let display = format!("Stimulus subscore: {}", clip.clip_id);
        let code = obx_code(profile, &local, &display);
        push(
            &mut out,
            code,
            ObxValue::Numeric {
                value: score,
                unit: "score",
            },
        );
    }
    if let Some(d) = review.and_then(|r| r.disposition) {
        let code = obx_code(profile, "disposition", "Clinician disposition");
        let mut label = d.as_str().replace('_', " ");
        label[..1].make_ascii_uppercase();
        push(
            &mut out,
            code,
            ObxValue::Coded(ObxCode {
                code: d.as_str(),
                text: &label,
                system: "L",
            }),
        );
    }
    if let Some(impression) = review.and_then(|r: &ClinicalReviewSummary| r.impression.as_deref()) {
        for paragraph in paragraphs(impression) {
            let code = obx_code(profile, "impression", "Clinician impression");
            push(&mut out, code, ObxValue::Text(paragraph));
        }
    }
    if profile.embed_pdf {
        let pdf = options.pdf.ok_or(Hl7Error::PdfMissing)?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(pdf);
        let code = obx_code(profile, "summary-pdf", "Clinical summary (PDF)");
        push(&mut out, code, ObxValue::Encapsulated { base64: &b64 });
    }
    Ok(out.into_bytes())
}

/// `OBX-3` for `local`: the profile's mapped code, else the local one (coding system `L`).
fn obx_code<'a>(profile: &'a Hl7Profile, local: &'a str, display: &'a str) -> ObxCode<'a> {
    match profile.code_map.get(local) {
        Some(m) => ObxCode {
            code: &m.code,
            text: &m.text,
            system: &m.system,
        },
        None => ObxCode {
            code: local,
            text: display,
            system: "L",
        },
    }
}

/// Measured value of a local metric code; absent or non-finite values are `None` (PLC-3).
fn metric_value(record: &ExportRecord, local: &str) -> Option<f64> {
    let s = &record.session;
    let v = match local {
        "social-preference-score" => s.social_preference,
        "eye-region-dwell-ratio" => s.eye_dwell,
        "mouth-region-dwell-ratio" => s.mouth_dwell,
        "valid-frame-ratio" => s.valid_frame_ratio,
        "composite-engagement" => s.summary.composite_score,
        other => {
            let key = AFFECT_KEYS.iter().find(|(l, _)| *l == other)?.1;
            s.affect.as_ref()?.get(key)?.as_f64()
        }
    };
    v.filter(|v| v.is_finite())
}

/// Non-empty paragraphs: blank-line separated blocks (single line breaks stay inside a
/// paragraph and are escaped as `\.br\`).
fn paragraphs(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .flat_map(|p| p.split("\r\n\r\n"))
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

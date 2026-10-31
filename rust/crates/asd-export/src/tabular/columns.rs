//! owner: WP-I.2
//!
//! Stable snake_case column names and their descriptions, one source for CSV, XLSX and
//! `manifest.json` (CSV-4). `export_id` is the first column of every table (CSV-3).
//!
//! The same module turns [`ExportRecord`]s into typed [`Cell`]s, so the dialects differ only
//! in how a cell is rendered, never in which values a table holds.

use chrono::{DateTime, NaiveDateTime, Offset, SecondsFormat, Utc};
use serde::Serialize;

use crate::model::ExportRecord;
use crate::ExportError;

/// One of the four tables of an export (CSV-3, XLSX-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Table {
    Sessions,
    Clips,
    MetricRows,
    Reviews,
}

impl Table {
    /// Every table, in ZIP and workbook order.
    pub const ALL: [Self; 4] = [Self::Sessions, Self::Clips, Self::MetricRows, Self::Reviews];

    /// File name inside the CSV ZIP (CSV-3).
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Sessions => "sessions.csv",
            Self::Clips => "clips.csv",
            Self::MetricRows => "metric_rows.csv",
            Self::Reviews => "reviews.csv",
        }
    }

    /// Sheet name in the workbook (XLSX-1).
    pub fn sheet_name(self) -> &'static str {
        match self {
            Self::Sessions => "Sessions",
            Self::Clips => "Clips",
            Self::MetricRows => "MetricRows",
            Self::Reviews => "Reviews",
        }
    }

    /// The table's columns, `export_id` first.
    pub fn columns(self) -> &'static [Column] {
        match self {
            Self::Sessions => SESSIONS,
            Self::Clips => CLIPS,
            Self::MetricRows => METRIC_ROWS,
            Self::Reviews => REVIEWS,
        }
    }
}

/// A column of the dictionary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Column {
    pub name: &'static str,
    pub description: &'static str,
}

const fn c(name: &'static str, description: &'static str) -> Column {
    Column { name, description }
}

const EXPORT_ID: Column = c(
    "export_id",
    "Identifier of this export (also in the watermark).",
);
const SESSION_ID: Column = c(
    "session_id",
    "Session identifier (a pseudonym when de-identified).",
);

static SESSIONS: &[Column] = &[
    EXPORT_ID,
    SESSION_ID,
    c(
        "participant_token",
        "Pseudonymous participant token (a pseudonym when de-identified).",
    ),
    c(
        "started_at",
        "Session start. csv: RFC 3339 UTC; csv_excel, csv_sheets and XLSX: local time \
         (YYYY-MM-DD HH:MM:SS) in the organisation time zone. YYYY-MM when de-identified.",
    ),
    c(
        "utc_offset",
        "UTC offset of started_at in the organisation time zone, e.g. +05:45; +00:00 in the \
         csv dialect; empty when started_at is reduced to a month.",
    ),
    c(
        "days_since_first_session",
        "Whole days since the participant's first session in this export (de-identified exports only).",
    ),
    c("duration_sec", "Capture duration in seconds."),
    c("battery", "Protocol id, e.g. express_30s."),
    c("capture_source", "truedepth, simulated or synthetic."),
    c("quality_verdict", "Capture quality verdict."),
    c(
        "attention_flag",
        "Screening attention flag: green, yellow, red or unreliable. Not a diagnosis.",
    ),
    c("composite_score", "Composite score of the session."),
    c("composite_confidence", "1 minus the share of low-confidence clips."),
    c("bundle_version", "Stimulus bundle version."),
    c(
        "device_id",
        "Capture device identifier (empty when de-identified).",
    ),
    c("org_id", "Organisation identifier."),
    c("age_pack", "Age pack used for the norms."),
    c("age_bracket_at_enrol", "Age bracket at enrolment."),
    c("lang", "Language of the session."),
    c("schema_version", "Session schema version."),
    c("valid_frame_ratio", "Share of frames with a valid gaze estimate."),
    c("calibrated_distance_cm", "Calibrated viewing distance in centimetres."),
    c("social_preference", "Social preference score."),
    c("eye_dwell", "Share of dwell time on the eyes."),
    c("mouth_dwell", "Share of dwell time on the mouth."),
    c(
        "first_eye_latency_ms",
        "Latency of the first fixation on the eyes in milliseconds.",
    ),
    c("reviewed", "A clinician review exists."),
    c("mrn", "Medical record number (identified exports only)."),
    c(
        "mrn_assigning_authority",
        "Hospital that assigned the MRN (identified exports only).",
    ),
    c("review_state", "State of the clinical review, empty when none."),
    c("report_status", "preliminary, final or amended."),
    c("disposition", "The clinician's disposition, empty when none."),
];

static CLIPS: &[Column] = &[
    EXPORT_ID,
    SESSION_ID,
    c("clip_id", "Stimulus clip identifier."),
    c("clip_order", "Position of the clip in the session."),
    c(
        "status",
        "Worst status of the clip's metric rows: onTarget, monitor or atypical.",
    ),
    c("score", "Clip score."),
    c("weight", "Clip weight in the composite."),
    c("dwell_social", "Dwell share on social areas of interest."),
    c(
        "dwell_geometric",
        "Dwell share on geometric areas of interest.",
    ),
    c("dwell_eye", "Dwell share on the eyes."),
    c("dwell_mouth", "Dwell share on the mouth."),
    c("orienting_latency_ms", "Orienting latency in milliseconds."),
    c("gaze_cue_accuracy", "Accuracy of following the gaze cue."),
    c("fixation_count", "Number of fixations."),
    c(
        "mean_fixation_ms",
        "Mean fixation duration in milliseconds.",
    ),
];

static METRIC_ROWS: &[Column] = &[
    EXPORT_ID,
    SESSION_ID,
    c("clip_id", "Stimulus clip identifier."),
    c("segment_id", "Segment of the clip the row scores."),
    c("metric", "Biomarker key, e.g. eye_dwell_ratio."),
    c("target_aoi", "Area of interest the metric refers to."),
    c("direction", "higherBetter, lowerBetter or band."),
    c("expected", "Expected value from the norms."),
    c("acceptable_low", "Lower bound of the acceptable range."),
    c("acceptable_high", "Upper bound of the acceptable range."),
    c("recorded", "Recorded value."),
    c("deviation", "recorded minus expected."),
    c("status", "onTarget, monitor or atypical."),
    c("score", "Row score."),
    c("imputed", "The value was imputed, not measured."),
    c("source", "Origin of the norm."),
];

static REVIEWS: &[Column] = &[
    EXPORT_ID,
    SESSION_ID,
    c(
        "review_state",
        "open, in_progress, awaiting_cosign, final or amending.",
    ),
    c("version_no", "Version of the review described."),
    c("amended", "The version is a signed amendment."),
    c("report_status", "preliminary, final or amended."),
    c("disposition", "The clinician's disposition."),
    c(
        "signed_at",
        "Time of the current signed state (same time rules as sessions.started_at).",
    ),
    c(
        "utc_offset",
        "UTC offset of signed_at (see sessions.utc_offset).",
    ),
    c(
        "content_hash",
        "SHA-256 of the signed snapshot (identified exports only).",
    ),
    c("signature_count", "Number of signatures."),
    c(
        "signers",
        "Signers' display names in signing order, separated by '; ' (identified exports only).",
    ),
    c(
        "registrations",
        "Signers' professional registrations, separated by '; ' (identified exports only).",
    ),
    c(
        "impression",
        "Clinical impression (when the requester may read it).",
    ),
    c(
        "recommendations",
        "Recommendations (when the requester may read it).",
    ),
];

/// A timestamp, kept typed until a dialect renders it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instant {
    pub utc: DateTime<Utc>,
    /// Wall-clock time in the organisation time zone.
    pub local: NaiveDateTime,
    /// `+05:45`.
    pub offset: String,
}

impl Instant {
    /// RFC 3339 UTC with milliseconds (a fixed width column).
    pub fn rfc3339_utc(&self) -> String {
        self.utc.to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    /// `YYYY-MM-DD HH:MM:SS` local time.
    pub fn local_text(&self) -> String {
        self.local.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

/// One cell of a table.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Absent value: an empty cell.
    Empty,
    /// Free or enumerated text. CSV applies the formula guard (CSV-2).
    Text(String),
    /// Text generated by this crate that is known safe and must stay verbatim, such as the
    /// `+05:45` offset (the guard would turn it into `'+05:45`).
    Verbatim(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Time(Instant),
}

fn text(s: &str) -> Cell {
    Cell::Text(s.to_owned())
}

fn opt_text(s: Option<&str>) -> Cell {
    s.map_or(Cell::Empty, text)
}

fn opt_f(v: Option<f64>) -> Cell {
    v.filter(|x| x.is_finite()).map_or(Cell::Empty, Cell::Float)
}

fn float(v: f64) -> Cell {
    opt_f(Some(v))
}

fn opt_i(v: Option<i64>) -> Cell {
    v.map_or(Cell::Empty, Cell::Int)
}

fn opt_b(v: Option<bool>) -> Cell {
    v.map_or(Cell::Empty, Cell::Bool)
}

/// The wire name of a serde enum (`onTarget`, `green`, ...).
fn wire<T: Serialize>(v: &T) -> Cell {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => Cell::Text(s),
        _ => Cell::Empty,
    }
}

/// An RFC 3339 timestamp in the organisation zone. De-identified records hold `YYYY-MM`
/// (XC-4): that is not a point in time, so it stays text with an empty offset.
fn time(field: &'static str, value: &str, tz: chrono_tz::Tz) -> Result<(Cell, Cell), ExportError> {
    if is_month(value) {
        return Ok((text(value), Cell::Empty));
    }
    let utc = DateTime::parse_from_rfc3339(value)
        .map_err(|_| ExportError::InvalidTimestamp { field })?
        .with_timezone(&Utc);
    let zoned = utc.with_timezone(&tz);
    let secs = zoned.offset().fix().local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let offset = format!(
        "{sign}{:02}:{:02}",
        secs.abs() / 3600,
        secs.abs() % 3600 / 60
    );
    Ok((
        Cell::Time(Instant {
            utc,
            local: zoned.naive_local(),
            offset: offset.clone(),
        }),
        Cell::Verbatim(offset),
    ))
}

fn is_month(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7
        && b[4] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..].iter().all(u8::is_ascii_digit)
}

/// All rows of `table`, one `Vec<Cell>` per row, matching [`Table::columns`].
pub fn rows(
    table: Table,
    export_id: &str,
    records: &[ExportRecord],
) -> Result<Vec<Vec<Cell>>, ExportError> {
    let mut out = Vec::new();
    for r in records {
        let tz = r.org.tz()?;
        let s = &r.session;
        let sm = &s.summary;
        let head = || vec![text(export_id), text(&sm.id)];
        match table {
            Table::Sessions => {
                let (started, offset) = time("session.started_at", &sm.started_at, tz)?;
                let rv = r.review.as_ref();
                let mrn = r.patient.mrn.as_ref();
                let mut row = head();
                row.extend([
                    text(&sm.participant_token),
                    started,
                    offset,
                    opt_i(r.days_since_first_session),
                    float(sm.duration_sec),
                    opt_text(sm.battery.as_deref()),
                    sm.capture_source.as_ref().map_or(Cell::Empty, wire),
                    text(&sm.quality_verdict),
                    wire(&sm.attention_flag),
                    opt_f(sm.composite_score),
                    opt_f(sm.composite_confidence),
                    opt_text(sm.bundle_version.as_deref()),
                    opt_text(sm.device_id.as_deref()),
                    text(&s.org_id),
                    opt_text(s.age_pack.as_deref()),
                    opt_text(r.patient.age_bracket_at_enrol.as_deref()),
                    opt_text(s.lang.as_deref()),
                    opt_text(s.schema_version.as_deref()),
                    opt_f(s.valid_frame_ratio),
                    opt_f(s.calibrated_distance_cm),
                    opt_f(s.social_preference),
                    opt_f(s.eye_dwell),
                    opt_f(s.mouth_dwell),
                    opt_f(s.first_eye_latency_ms),
                    opt_b(sm.reviewed),
                    opt_text(mrn.map(|m| m.value.as_str())),
                    opt_text(mrn.map(|m| m.assigning_authority.as_str())),
                    rv.map_or(Cell::Empty, |v| wire(&v.state)),
                    wire(&r.report_status()),
                    rv.and_then(|v| v.disposition)
                        .map_or(Cell::Empty, |d| wire(&d)),
                ]);
                out.push(row);
            }
            Table::Clips => {
                for k in &r.clips {
                    let mut row = head();
                    row.extend([
                        text(&k.clip_id),
                        Cell::Int(k.order),
                        k.status.as_ref().map_or(Cell::Empty, wire),
                        opt_f(k.score),
                        opt_f(k.weight),
                        opt_f(k.dwell_social),
                        opt_f(k.dwell_geometric),
                        opt_f(k.dwell_eye),
                        opt_f(k.dwell_mouth),
                        opt_f(k.orienting_latency_ms),
                        opt_f(k.gaze_cue_accuracy),
                        opt_i(k.fixation_count),
                        opt_f(k.mean_fixation_ms),
                    ]);
                    out.push(row);
                }
            }
            Table::MetricRows => {
                for k in &r.clips {
                    for m in &k.rows {
                        let mut row = head();
                        row.extend([
                            text(&k.clip_id),
                            text(&m.segment_id),
                            text(&m.metric),
                            opt_text(m.target_aoi.as_deref()),
                            wire(&m.direction),
                            opt_f(m.expected),
                            opt_f(m.acceptable_low),
                            opt_f(m.acceptable_high),
                            float(m.recorded),
                            opt_f(m.deviation),
                            wire(&m.status),
                            float(m.score),
                            Cell::Bool(m.imputed),
                            text(&m.source),
                        ]);
                        out.push(row);
                    }
                }
            }
            Table::Reviews => {
                let Some(v) = r.review.as_ref() else {
                    continue;
                };
                let (signed, offset) = match v.signed_at.as_deref() {
                    Some(t) => time("review.signed_at", t, tz)?,
                    None => (Cell::Empty, Cell::Empty),
                };
                let join = |f: fn(&crate::model::Signature) -> Option<&str>| {
                    let parts: Vec<&str> = v.signatures.iter().filter_map(f).collect();
                    if parts.is_empty() {
                        Cell::Empty
                    } else {
                        Cell::Text(parts.join("; "))
                    }
                };
                let mut row = head();
                row.extend([
                    wire(&v.state),
                    Cell::Int(i64::from(v.version_no)),
                    Cell::Bool(v.amended),
                    wire(&v.report_status()),
                    v.disposition.map_or(Cell::Empty, |d| wire(&d)),
                    signed,
                    offset,
                    opt_text(v.content_hash.as_deref()),
                    Cell::Int(i64::try_from(v.signatures.len()).unwrap_or(i64::MAX)),
                    join(|s| s.display_name.as_deref()),
                    join(|s| s.professional_registration.as_deref()),
                    opt_text(v.impression.as_deref()),
                    opt_text(v.recommendations.as_deref()),
                ]);
                out.push(row);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_starts_with_export_id_and_names_are_unique_snake_case() {
        for t in Table::ALL {
            let cols = t.columns();
            assert_eq!(cols[0].name, "export_id");
            let mut seen = std::collections::BTreeSet::new();
            for col in cols {
                assert!(seen.insert(col.name), "{} duplicated in {t:?}", col.name);
                assert!(col
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'));
                assert!(!col.description.is_empty());
            }
        }
    }

    #[test]
    fn month_detection() {
        assert!(is_month("2026-10"));
        assert!(!is_month("2026-10-01"));
        assert!(!is_month("20x6-10"));
    }
}

//! owner: WP-G.1
//!
//! The raw tracking log (REP-1): JSON Lines, UTF-8, `\n` line ends.
//!
//! - Line 1: a [`RawLogHeader`] whose `format` is [`RAW_LOG_FORMAT`].
//! - Every further line: one [`RawTrackingSample`] (PIPE-2, snake_case keys), with `t_sec`
//!   strictly increasing.
//! - Blank lines are ignored; line numbers in errors are 1-based physical lines.
//!
//! [`to_jsonl`] is the canonical writer (serde field order, one object per line, trailing
//! newline); `asd-cli gaze replay --check` (WP-G.5) is built on [`parse`].

use crate::types::tracking::{RawLogHeader, RawTrackingSample, RAW_LOG_FORMAT};

/// A parsed raw log.
#[derive(Debug, Clone, PartialEq)]
pub struct RawLog {
    pub header: RawLogHeader,
    pub samples: Vec<RawTrackingSample>,
}

/// Why a raw log was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RawLogError {
    #[error("raw log is empty")]
    Empty,
    #[error("line {line}: first line is not a header (no `format` key)")]
    MissingHeader { line: usize },
    #[error("unsupported raw log format `{found}` (expected `{RAW_LOG_FORMAT}`)")]
    UnsupportedFormat { found: String },
    #[error("line {line}: {message}")]
    Json { line: usize, message: String },
    #[error("line {line}: t_sec is not strictly increasing")]
    NonMonotonicTime { line: usize },
}

/// Parses and checks a raw log.
pub fn parse(text: &str) -> Result<RawLog, RawLogError> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l))
        .filter(|(_, l)| !l.trim().is_empty());

    let (hline, htext) = lines.next().ok_or(RawLogError::Empty)?;
    let hvalue: serde_json::Value = serde_json::from_str(htext).map_err(|e| RawLogError::Json {
        line: hline,
        message: e.to_string(),
    })?;
    let format = match hvalue.get("format") {
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => return Err(RawLogError::MissingHeader { line: hline }),
    };
    if format != RAW_LOG_FORMAT {
        return Err(RawLogError::UnsupportedFormat { found: format });
    }
    let header: RawLogHeader = serde_json::from_value(hvalue).map_err(|e| RawLogError::Json {
        line: hline,
        message: e.to_string(),
    })?;

    let mut samples: Vec<RawTrackingSample> = Vec::new();
    for (line, l) in lines {
        let s: RawTrackingSample = serde_json::from_str(l).map_err(|e| RawLogError::Json {
            line,
            message: e.to_string(),
        })?;
        if let Some(prev) = samples.last() {
            if s.t_sec.partial_cmp(&prev.t_sec) != Some(std::cmp::Ordering::Greater) {
                return Err(RawLogError::NonMonotonicTime { line });
            }
        }
        samples.push(s);
    }
    Ok(RawLog { header, samples })
}

/// Writes a log in canonical form (header line, one sample per line, trailing newline).
pub fn to_jsonl(log: &RawLog) -> Result<String, serde_json::Error> {
    let mut out = serde_json::to_string(&log.header)?;
    out.push('\n');
    for s in &log.samples {
        out.push_str(&serde_json::to_string(s)?);
        out.push('\n');
    }
    Ok(out)
}

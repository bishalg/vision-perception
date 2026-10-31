//! owner: WP-I.1
//!
//! The export watermark (XC-6): every artifact names its export, who requested it and when,
//! the mode and the purpose. Formats place it themselves (CSV-3 manifest, XLSX-2
//! properties and header/footer, PDF-4 footer, FHX-7 `Bundle.identifier`/`meta.tag`, HL7-6
//! NTE) and use [`Watermark::line`] for the human-readable text, so the wording is one
//! implementation. Every human-readable artifact also carries [`MANDATORY_SENTENCE`]
//! (ADR-0004); each encoder tests for it because the Swift claims scanner does not see
//! Rust strings (G16).

use serde::{Deserialize, Serialize};

use crate::claims;
use crate::policy::{Mode, Purpose};

/// "This is not a diagnostic tool." (ADR-0004, CLM-5).
pub const MANDATORY_SENTENCE: &str = claims::NOT_DIAGNOSTIC;

/// Separator between watermark fields (as in the PDF-4 footer).
const SEP: &str = " · ";

/// Facts printed on every artifact of one export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watermark {
    /// `exports.id`.
    pub export_id: String,
    /// Requester's display name (NOTE-6), or their e-mail when none is set.
    pub requested_by_display: String,
    /// RFC 3339 UTC, as stored on the export row.
    pub requested_at_utc: String,
    pub mode: Mode,
    pub purpose: Purpose,
    pub org_name: String,
}

impl Watermark {
    /// One line, no control characters (CR/LF/TAB in a name become spaces, so a watermark
    /// can never split a CSV row, an HL7 segment or a footer line):
    /// `Export {id} · {org} · {mode} · {purpose} · requested by {name} · {requested_at}`.
    pub fn line(&self) -> String {
        [
            format!("Export {}", one_line(&self.export_id)),
            one_line(&self.org_name),
            self.mode.label().to_owned(),
            self.purpose.label().to_owned(),
            format!("requested by {}", one_line(&self.requested_by_display)),
            one_line(&self.requested_at_utc),
        ]
        .join(SEP)
    }

    /// The watermark line followed by the mandatory sentence, for formats that print text
    /// blocks (HL7 NTE, XLSX README, CSV manifest).
    pub fn lines(&self) -> [String; 2] {
        [self.line(), MANDATORY_SENTENCE.to_owned()]
    }
}

/// Control characters become single spaces; runs of them collapse; ends are trimmed.
fn one_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for ch in s.chars() {
        if ch.is_control() || ch.is_whitespace() {
            pending_space = true;
        } else {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        }
    }
    out
}

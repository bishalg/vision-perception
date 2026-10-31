//! owner: WP-I.1
//!
//! De-identification (XC-4), applied by the crate whenever `mode = deidentified`, before
//! any encoder sees the records:
//!
//! - MRN removed;
//! - participant id and token replaced by `P-` + the first 10 RFC 4648 base32 characters
//!   of `HMAC-SHA256(salt, participant_id)`; session id by `S-` + the same over the
//!   session id;
//! - timestamps reduced to `YYYY-MM` (UTC month), plus `days_since_first_session`: whole
//!   days since the participant's earliest session in this export;
//! - impression, recommendations, notes and letters removed; signers' user ids, names and
//!   registrations removed (signatures, roles, state and disposition kept); the content
//!   hash removed (it would link back to the identified review);
//! - device id removed; `age_bracket_at_enrol`, measurements and clips kept.
//!
//! The salt is random per export (the caller draws it; this crate has no randomness) and
//! must not be stored after the job finishes, so pseudonyms cannot be linked across
//! exports.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::model::ExportRecord;
use crate::ExportError;

/// Length of the per-export salt.
pub const SALT_LEN: usize = 32;
/// Base32 characters kept after the prefix.
const PSEUDONYM_CHARS: usize = 10;
const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// `prefix` + first 10 base32 characters of `HMAC-SHA256(salt, input)`.
pub fn pseudonym(salt: &[u8; SALT_LEN], prefix: &str, input: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(salt).expect("HMAC accepts any key length");
    mac.update(input.as_bytes());
    let digest = mac.finalize().into_bytes();
    let mut out = String::with_capacity(prefix.len() + PSEUDONYM_CHARS);
    out.push_str(prefix);
    out.push_str(&base32(&digest, PSEUDONYM_CHARS));
    out
}

/// The first `chars` characters of the RFC 4648 base32 encoding of `bytes`.
fn base32(bytes: &[u8], chars: usize) -> String {
    let mut out = String::with_capacity(chars);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(BASE32[((buffer >> bits) & 31) as usize]));
            if out.len() == chars {
                return out;
            }
        }
        buffer &= (1 << bits) - 1;
    }
    out
}

fn parse(field: &'static str, rfc3339: &str) -> Result<DateTime<Utc>, ExportError> {
    DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| ExportError::InvalidTimestamp { field })
}

fn month(field: &'static str, rfc3339: &str) -> Result<String, ExportError> {
    Ok(parse(field, rfc3339)?.format("%Y-%m").to_string())
}

/// De-identified copies of `records`, in the same order. The input is not modified.
pub fn apply(
    records: &[ExportRecord],
    salt: &[u8; SALT_LEN],
) -> Result<Vec<ExportRecord>, ExportError> {
    // Earliest session per participant in this export.
    let mut first: BTreeMap<&str, DateTime<Utc>> = BTreeMap::new();
    for r in records {
        let t = parse("session.started_at", &r.session.summary.started_at)?;
        first
            .entry(r.patient.participant_id.as_str())
            .and_modify(|f| *f = (*f).min(t))
            .or_insert(t);
    }

    records
        .iter()
        .map(|r| {
            let started = parse("session.started_at", &r.session.summary.started_at)?;
            let days = (started - first[r.patient.participant_id.as_str()]).num_days();
            let p = pseudonym(salt, "P-", &r.patient.participant_id);
            let s = pseudonym(salt, "S-", &r.session.summary.id);

            let mut out = r.clone();
            out.days_since_first_session = Some(days);

            out.patient.mrn = None;
            out.patient.participant_id.clone_from(&p);
            out.patient.participant_token.clone_from(&p);

            let summary = &mut out.session.summary;
            summary.id = s;
            summary.participant_id.clone_from(&p);
            summary.participant_token = p;
            summary.started_at = started.format("%Y-%m").to_string();
            summary.device_id = None;

            if let Some(review) = out.review.as_mut() {
                review.impression = None;
                review.recommendations = None;
                review.content_hash = None;
                review.signed_at = review
                    .signed_at
                    .as_deref()
                    .map(|t| month("review.signed_at", t))
                    .transpose()?;
                for sig in &mut review.signatures {
                    sig.user_id = None;
                    sig.display_name = None;
                    sig.professional_registration = None;
                    sig.signed_at = month("signature.signed_at", &sig.signed_at)?;
                }
            }
            out.notes.clear();
            out.letters.clear();
            Ok(out)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_matches_rfc4648_vectors() {
        // RFC 4648 section 10: BASE32("fooba") = "MZXW6YTB", BASE32("foobar") =
        // "MZXW6YTBOI======". Only whole 5-bit groups are emitted (48 bits -> 9), which is
        // all pseudonyms need: they take 10 of the 51 groups of a 32-byte digest.
        assert_eq!(base32(b"fooba", 8), "MZXW6YTB");
        assert_eq!(base32(b"foobar", 10), "MZXW6YTBO");
    }
}

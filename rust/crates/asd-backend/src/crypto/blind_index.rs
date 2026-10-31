//! owner: WP-C.9
//!
//! KEY-4: exact-match lookup of an MRN without storing it in clear.
//!
//! ```text
//! mrn_bidx        = HMAC-SHA256(org_index_key, normalize(authority) || 0x1F || normalize(mrn))
//! org_index_key   = HKDF-SHA256(DEK, info = "mrn-bidx-v1")
//! normalize(s)    = trim, NFKC, upper-case, remove whitespace and hyphens
//! ```
//!
//! The index key comes from the org's first data key (see
//! [`super::Keyring::mrn_blind_index`]), so it does not change when a new DEK is rotated in
//! and the unique index on `patient_refs` keeps working. Different orgs have different
//! DEKs, so the same MRN gives unrelated indexes in two orgs.

use aws_lc_rs::{hkdf, hmac};
use unicode_normalization::UnicodeNormalization;

/// HKDF `info` for the index key; bump the suffix to re-key every index.
pub const INFO: &[u8] = b"mrn-bidx-v1";
/// Separator between the normalised authority and MRN (ASCII unit separator).
pub const SEPARATOR: u8 = 0x1F;

/// Hyphen-like code points removed besides ASCII `-` (NFKC keeps most of them distinct).
fn is_hyphen(c: char) -> bool {
    matches!(
        c,
        '-' | '\u{2010}'..='\u{2015}' | '\u{2212}' | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}'
    )
}

/// `normalize` of KEY-4: trim, NFKC, upper-case, drop whitespace and hyphens.
pub fn normalize(s: &str) -> String {
    s.trim()
        .nfkc()
        .flat_map(char::to_uppercase)
        .filter(|c| !c.is_whitespace() && !is_hyphen(*c))
        .collect()
}

/// The org's HMAC key derived from its data key bytes.
pub(crate) fn index_key(dek: &[u8; 32]) -> hmac::Key {
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(dek);
    // A fixed 32-byte output length is always within HKDF's limit.
    prk.expand(&[INFO], hmac::HMAC_SHA256)
        .expect("HKDF-SHA256 expands 32 bytes")
        .into()
}

/// The blind index of `(authority, mrn)` under `key`.
pub(crate) fn compute(key: &hmac::Key, authority: &str, mrn: &str) -> [u8; 32] {
    let mut msg = normalize(authority).into_bytes();
    msg.push(SEPARATOR);
    msg.extend_from_slice(normalize(mrn).as_bytes());
    let tag = hmac::sign(key, &msg);
    let mut out = [0u8; 32];
    out.copy_from_slice(tag.as_ref());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_keyed() {
        let k1 = index_key(&[1; 32]);
        let k2 = index_key(&[2; 32]);
        assert_eq!(
            compute(&k1, "TUTH", "12-34"),
            compute(&k1, " tuth", "1234 ")
        );
        assert_ne!(compute(&k1, "TUTH", "1234"), compute(&k2, "TUTH", "1234"));
        assert_ne!(compute(&k1, "AB", "C"), compute(&k1, "A", "BC"));
    }
}

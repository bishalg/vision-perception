//! owner: WP-I.13
//!
//! Case-package fixtures (SHR-7): a participant with two exportable sessions, a session
//! note and an MRN; another participant; request bodies; and the archive search of
//! WP-I.12's de-identification test (`asd-export/tests/case_package.rs`), reused here on
//! the job's real artifacts.
#![allow(dead_code)]

use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use super::unzip;
use crate::participant_support::Ctx;

pub const TOKEN: &str = "TOKEN-CASE-SENTINEL";
pub const SESSION_1: &str = "SESS-CASE-ONE";
pub const SESSION_2: &str = "SESS-CASE-TWO";
pub const OTHER_TOKEN: &str = "TOKEN-OTHER";
pub const OTHER_SESSION: &str = "SESS-OTHER";
pub const NOTE: &str = "NOTE-SENTINEL observed good joint attention";
pub const MRN: &str = "MRN-SENTINEL-778899";
pub const RECIPIENT: &str = "Dr Recipient-Sentinel, City Hospital";
pub const CONSENT_REF: &str = "CONSENT-FORM-SENTINEL-42";
pub const CONSENT_AT: &str = "2026-10-20T09:00:00Z";

/// The case participant and the other one.
pub struct CaseFixture {
    pub participant: Uuid,
    pub other: Uuid,
}

impl Ctx {
    /// `TOKEN`: `SESSION_1` (research consent) with a clinical note, `SESSION_2` (no
    /// research consent recorded), an MRN (the org in MRN mode); `OTHER_TOKEN` with one
    /// session. Notes and the MRN are written through the API by `clinician`.
    pub async fn case_fixture(&self, clinician: &str) -> CaseFixture {
        let participant = self.participant(self.org, TOKEN).await;
        let other = self.participant(self.org, OTHER_TOKEN).await;
        self.exportable_session(
            participant,
            TOKEN,
            SESSION_1,
            "2026-10-01T10:00:00Z",
            Some(true),
        )
        .await;
        self.exportable_session(participant, TOKEN, SESSION_2, "2026-10-03T10:00:00Z", None)
            .await;
        self.exportable_session(
            other,
            OTHER_TOKEN,
            OTHER_SESSION,
            "2026-10-02T10:00:00Z",
            None,
        )
        .await;
        let (s, b) = self
            .call(
                Method::POST,
                &format!("/v1/participants/{participant}/notes"),
                Some(json!({"body": NOTE, "session_id": SESSION_1})),
                clinician,
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        self.set_mode(self.org, "mrn").await;
        let (s, b) = self
            .call(
                Method::PUT,
                &format!("/v1/participants/{participant}/patient-ref"),
                Some(json!({"assigning_authority": "TUTH", "mrn": MRN})),
                clinician,
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        CaseFixture { participant, other }
    }

    /// Requests an identified clinical-care case package of `participant`, runs the
    /// queue and returns the finished export.
    pub async fn ready_case(&self, bearer: &str, participant: Uuid) -> Value {
        let done = self
            .export_and_run(
                bearer,
                case_body("identified", "clinical_care", participant),
            )
            .await;
        assert_eq!(done["status"], "succeeded", "{done}");
        done
    }
}

/// A complete `case_package` request (consent included).
pub fn case_body(mode: &str, purpose: &str, participant: Uuid) -> Value {
    json!({
        "format": "case_package",
        "mode": mode,
        "purpose": purpose,
        "reason": "Referral to the developmental clinic",
        "case": {
            "participant_id": participant,
            "recipient": RECIPIENT,
            "consent_reference": CONSENT_REF,
            "consent_recorded_at": CONSENT_AT,
        },
    })
}

/// Entry names and contents of an archive, as one byte string (`name\0data\0` ...).
pub fn archive_bytes(zip: &[u8]) -> Vec<u8> {
    let mut all = Vec::new();
    for e in unzip::entries(zip) {
        all.extend_from_slice(e.name.as_bytes());
        all.push(0);
        all.extend_from_slice(&e.data);
        all.push(0);
    }
    all
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// WP-I.12's forbidden-string search (SHR-3): none of `needles` appears in any entry
/// name or decompressed content, nor in the raw ZIP bytes.
pub fn assert_archive_clean(zip: &[u8], needles: &[&str]) {
    let all = archive_bytes(zip);
    assert!(
        !all.is_empty(),
        "empty archive: the search would be vacuous"
    );
    for n in needles {
        assert!(!contains(&all, n.as_bytes()), "found {n:?} in the archive");
        assert!(!contains(zip, n.as_bytes()), "found {n:?} in the ZIP bytes");
    }
}

/// The control of [`assert_archive_clean`]: every one of `needles` is in the archive.
pub fn assert_archive_has(zip: &[u8], needles: &[&str]) {
    let all = archive_bytes(zip);
    for n in needles {
        assert!(
            contains(&all, n.as_bytes()),
            "{n:?} missing from the archive"
        );
    }
}

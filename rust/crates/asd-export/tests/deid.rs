//! XC-4 de-identification, applied by the crate whenever `mode = deidentified`.

mod export_support;

use asd_export::deid::{self, pseudonym};
use asd_export::model::{Disposition, ReviewState, SignatureRole};
use export_support::{
    full_record, golden_record, later_session, sentinel, DEVICE_ID, PARTICIPANT_ID,
};
use serde_json::Value;

const SALT_A: [u8; 32] = [7u8; 32];
const SALT_B: [u8; 32] = [8u8; 32];

fn is_base32(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b))
}

#[test]
fn pseudonym_is_prefix_plus_ten_base32_chars_of_hmac_sha256() {
    let p = pseudonym(&SALT_A, "P-", PARTICIPANT_ID);
    assert_eq!(p.len(), 12, "{p}");
    assert!(p.starts_with("P-"));
    assert!(is_base32(&p[2..]), "{p}");
    // Known answer from an independent implementation (Python):
    // base64.b32encode(hmac.new(bytes([7] * 32), b"abc", hashlib.sha256).digest())[:10]
    assert_eq!(pseudonym(&SALT_A, "S-", "abc"), "S-CJ62JTWWVA");
}

#[test]
fn every_xc4_transformation() {
    let original = full_record();
    let out = deid::apply(std::slice::from_ref(&original), &SALT_A).unwrap();
    assert_eq!(out.len(), 1);
    let r = &out[0];

    // MRN removed.
    assert!(r.patient.mrn.is_none());
    // Participant token and id -> P- pseudonym, everywhere they appear.
    let p = pseudonym(&SALT_A, "P-", PARTICIPANT_ID);
    assert_eq!(r.patient.participant_token, p);
    assert_eq!(r.patient.participant_id, p);
    assert_eq!(r.session.summary.participant_token, p);
    assert_eq!(r.session.summary.participant_id, p);
    // Session id -> S- pseudonym over the session id.
    assert_eq!(
        r.session.summary.id,
        pseudonym(&SALT_A, "S-", &original.session.summary.id)
    );
    // Timestamps reduced to YYYY-MM, plus days since the participant's first session.
    assert_eq!(r.session.summary.started_at, "2026-10");
    assert_eq!(r.days_since_first_session, Some(0));
    // Device id removed.
    assert!(r.session.summary.device_id.is_none());
    // Free text removed.
    let review = r.review.as_ref().expect("review summary kept");
    assert!(review.impression.is_none());
    assert!(review.recommendations.is_none());
    assert!(r.notes.is_empty());
    assert!(r.letters.is_empty());
    // Signers anonymised; signed flag, roles, state and disposition kept.
    assert_eq!(review.state, ReviewState::Final);
    assert_eq!(review.disposition, Some(Disposition::Monitor));
    assert!(review.amended);
    assert!(review.is_signed());
    assert!(review.content_hash.is_none());
    assert_eq!(review.signed_at.as_deref(), Some("2026-10"));
    assert_eq!(review.signatures.len(), 2);
    for (sig, role) in review
        .signatures
        .iter()
        .zip([SignatureRole::Sign, SignatureRole::Cosign])
    {
        assert_eq!(sig.role, role);
        assert!(sig.user_id.is_none());
        assert!(sig.display_name.is_none());
        assert!(sig.professional_registration.is_none());
        assert_eq!(sig.signed_at, "2026-10");
    }
    // Kept: age bracket, measurements, clips.
    assert_eq!(r.patient.age_bracket_at_enrol.as_deref(), Some("24-36m"));
    assert_eq!(
        r.session.social_preference,
        original.session.social_preference
    );
    assert_eq!(r.clips, original.clips);
    assert_eq!(r.consent, original.consent);
}

#[test]
fn input_is_not_modified() {
    let original = full_record();
    let copy = original.clone();
    let _ = deid::apply(std::slice::from_ref(&original), &SALT_A).unwrap();
    assert_eq!(original, copy);
}

#[test]
fn same_participant_in_one_export_maps_to_one_pseudonym() {
    let records = [
        later_session(30, "SESS-B"),
        full_record(),
        later_session(3, "SESS-C"),
    ];
    let out = deid::apply(&records, &SALT_A).unwrap();
    let tokens: Vec<&str> = out
        .iter()
        .map(|r| r.patient.participant_token.as_str())
        .collect();
    assert!(tokens.iter().all(|t| *t == tokens[0]), "{tokens:?}");
    // Distinct sessions keep distinct pseudonyms.
    let sessions: std::collections::BTreeSet<&str> =
        out.iter().map(|r| r.session.summary.id.as_str()).collect();
    assert_eq!(sessions.len(), 3);
    // Days counted from the participant's earliest session in the export, in input order.
    let days: Vec<Option<i64>> = out.iter().map(|r| r.days_since_first_session).collect();
    assert_eq!(days, [Some(30), Some(0), Some(3)]);
}

#[test]
fn different_participants_get_different_pseudonyms() {
    let a = golden_record();
    let mut b = golden_record();
    b.patient.participant_id = "another-participant".into();
    b.session.summary.participant_id = "another-participant".into();
    let out = deid::apply(&[a, b], &SALT_A).unwrap();
    assert_ne!(
        out[0].patient.participant_token,
        out[1].patient.participant_token
    );
    assert_eq!(out[1].days_since_first_session, Some(0));
}

#[test]
fn two_exports_with_different_salts_give_different_pseudonyms() {
    let a = deid::apply(&[full_record()], &SALT_A).unwrap();
    let b = deid::apply(&[full_record()], &SALT_B).unwrap();
    assert_ne!(
        a[0].patient.participant_token,
        b[0].patient.participant_token
    );
    assert_ne!(a[0].session.summary.id, b[0].session.summary.id);
    // Same salt, same output (deterministic, XC-8).
    let again = deid::apply(&[full_record()], &SALT_A).unwrap();
    assert_eq!(a, again);
}

#[test]
fn no_de_identified_field_survives_anywhere_in_the_serialised_record() {
    let original = full_record();
    let records = [original.clone(), later_session(10, "SESS-OTHER")];
    let out = deid::apply(&records, &SALT_A).unwrap();
    let json = serde_json::to_value(&out).unwrap();

    let forbidden = [
        sentinel::MRN,
        sentinel::AUTHORITY,
        sentinel::IMPRESSION,
        sentinel::RECOMMENDATIONS,
        sentinel::NOTE,
        sentinel::NOTE_AUTHOR,
        sentinel::LETTER_BODY,
        sentinel::LETTER_RECIPIENT,
        sentinel::SIGNER,
        sentinel::COSIGNER,
        sentinel::REGISTRATION,
        sentinel::SIGNER_ID,
        "cosigner-id-sentinel",
        sentinel::CONTENT_HASH,
        PARTICIPANT_ID,
        DEVICE_ID,
        original.patient.participant_token.as_str(),
        original.session.summary.id.as_str(),
        "SESS-OTHER",
    ];
    let mut strings = Vec::new();
    collect_strings(&json, &mut strings);
    for s in &strings {
        for f in forbidden {
            assert!(!s.contains(f), "{f:?} survived in {s:?}");
        }
        // No full timestamp survives: only `YYYY-MM`.
        assert!(!looks_like_timestamp(s), "timestamp survived: {s:?}");
    }
}

fn looks_like_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(11).any(|w| {
        w[..4].iter().all(u8::is_ascii_digit)
            && w[4] == b'-'
            && w[5..7].iter().all(u8::is_ascii_digit)
            && w[7] == b'-'
            && w[8..10].iter().all(u8::is_ascii_digit)
            && (w[10] == b'T' || w[10] == b' ')
    })
}

fn collect_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Object(map) => {
            for (k, x) in map {
                out.push(k.clone());
                collect_strings(x, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|x| collect_strings(x, out)),
        _ => {}
    }
}

#[test]
fn an_invalid_timestamp_is_an_error_not_a_guess() {
    let mut r = golden_record();
    r.session.summary.started_at = "yesterday".into();
    assert!(deid::apply(&[r], &SALT_A).is_err());
}

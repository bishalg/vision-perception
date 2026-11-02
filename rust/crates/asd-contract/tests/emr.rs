//! owner: WP-R.7
//!
//! REC-9 (acceptance 25): the EMR-lite and share-a-case DTOs round-trip, accept older
//! bodies without the optional fields, are published in `api-v1.json`, and the REC-12 QR
//! payload builds and parses symmetrically.

use asd_contract::api_v1::Participant;
use asd_contract::emr::{
    self, Attachment, AttachmentKind, AttachmentMediaType, AttachmentUpload, CaseShareRequest,
    CreateAttachmentRequest, CreateEncounterRequest, CreateParticipantRequest,
    CreateShareLinkRequest, CreatedParticipant, CreatedShareLink, Encounter, EncounterKind,
    EncounterPage, EncounterStatus, FollowUp, FollowUpPage, LinkSessionRequest,
    ParticipantIdentity, ParticipantSearchPage, ParticipantSearchRequest, ParticipantSearchRow,
    QrParseError, ShareLink, ShareLinkOpenRequest, ShareLinkOpenResponse, ShareLinkPage,
    UpdateEncounterRequest, UploadTarget,
};
use asd_contract::{problem, schema};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

const ORG: &str = "6f1c2a4e-8b3d-4f5a-9c7e-1d2b3c4d5e6f";
const PID: &str = "0b9e8d7c-6a5f-4e3d-8c2b-1a0f9e8d7c6b";
const TOKEN: &str = "child-0123456789abcdef0123";
const TS: &str = "2026-11-02T09:30:00Z";

fn round_trip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_value(value).unwrap();
    let back: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(&back, value, "round-trip changed {json}");
}

fn participant() -> Participant {
    Participant {
        id: PID.into(),
        org_id: ORG.into(),
        participant_token: TOKEN.into(),
        age_bracket_at_enrol: Some("24-36m".into()),
        language: Some("ne".into()),
        session_count: 2,
        last_session_at: Some(TS.into()),
        withdrawn_at: None,
        has_patient_ref: Some(true),
    }
}

fn encounter() -> Encounter {
    Encounter {
        id: "e1".into(),
        participant_id: PID.into(),
        kind: EncounterKind::FollowUpVisit,
        occurred_at: TS.into(),
        clinician_id: Some("u1".into()),
        clinician_display_name: Some("Dr A".into()),
        reason: Some("six-month follow-up".into()),
        summary: Some("text".into()),
        has_summary: true,
        status: EncounterStatus::Open,
        follow_up_due_at: Some(TS.into()),
        follow_up_done_at: None,
        session_ids: vec!["s1".into(), "s2".into()],
        row_version: 3,
        created_at: TS.into(),
        updated_at: Some(TS.into()),
    }
}

fn attachment() -> Attachment {
    Attachment {
        id: "a1".into(),
        participant_id: PID.into(),
        encounter_id: Some("e1".into()),
        kind: AttachmentKind::PhotoOfDocument,
        file_name: "form_1.jpg".into(),
        media_type: AttachmentMediaType::ImageJpeg,
        byte_size: 2048,
        sha256: Some("ab".repeat(32)),
        uploaded_by: Some("u1".into()),
        created_at: TS.into(),
    }
}

fn share_link() -> ShareLink {
    ShareLink {
        id: "l1".into(),
        export_id: "x1".into(),
        url: "https://clinic.example/share/l1".into(),
        expires_at: TS.into(),
        max_downloads: 3,
        downloads: 1,
        opens: 2,
        revoked_at: None,
        created_at: TS.into(),
        last_opened_at: Some(TS.into()),
    }
}

#[test]
fn every_type_round_trips() {
    round_trip(&encounter());
    round_trip(&Encounter {
        summary: None,
        ..encounter()
    });
    round_trip(&CreateEncounterRequest {
        kind: EncounterKind::ScreeningVisit,
        occurred_at: TS.into(),
        clinician_id: Some("u1".into()),
        reason: Some("first visit".into()),
        summary: Some("text".into()),
        follow_up_due_at: Some(TS.into()),
        session_ids: vec!["s1".into()],
    });
    round_trip(&UpdateEncounterRequest {
        kind: Some(EncounterKind::HomeVisit),
        occurred_at: Some(TS.into()),
        clinician_id: None,
        reason: Some("r".into()),
        summary: None,
        status: Some(EncounterStatus::Closed),
        follow_up_due_at: None,
        follow_up_done_at: Some(TS.into()),
    });
    round_trip(&EncounterPage {
        items: vec![encounter()],
        next_cursor: Some("c".into()),
        total: Some(1),
    });
    round_trip(&LinkSessionRequest {
        session_id: "s1".into(),
    });
    let follow_up = FollowUp {
        encounter_id: "e1".into(),
        participant_id: PID.into(),
        participant_token: TOKEN.into(),
        kind: EncounterKind::PhoneCall,
        occurred_at: TS.into(),
        follow_up_due_at: TS.into(),
        clinician_id: None,
        clinician_display_name: None,
        reason: None,
    };
    round_trip(&follow_up);
    round_trip(&FollowUpPage {
        items: vec![follow_up],
        next_cursor: None,
        total: None,
    });
    round_trip(&attachment());
    round_trip(&CreateAttachmentRequest {
        kind: AttachmentKind::Referral,
        file_name: "referral.pdf".into(),
        media_type: AttachmentMediaType::ApplicationPdf,
        byte_size: 1,
        encounter_id: None,
    });
    round_trip(&AttachmentUpload {
        attachment: attachment(),
        upload: UploadTarget {
            method: "PUT".into(),
            url: "/v1/attachments/a1/content".into(),
        },
    });
    round_trip(&ParticipantSearchRequest {
        token_prefix: Some("child-01".into()),
        mrn: Some("12345".into()),
        age_bracket: Some("24-36m".into()),
        last_seen_from: Some(TS.into()),
        last_seen_to: Some(TS.into()),
        has_open_follow_up: Some(true),
        cursor: Some("c".into()),
        limit: Some(50),
    });
    let row = ParticipantSearchRow {
        participant: participant(),
        open_encounters_n: 1,
        next_follow_up_at: Some(TS.into()),
    };
    round_trip(&row);
    round_trip(&ParticipantSearchPage {
        items: vec![row],
        next_cursor: None,
        total: Some(1),
    });
    round_trip(&CreateParticipantRequest {
        age_bracket_at_enrol: Some("24-36m".into()),
        language: Some("en".into()),
        consent_version: Some("2026-10".into()),
        consent_at: Some(TS.into()),
    });
    round_trip(&CreatedParticipant {
        participant: participant(),
        qr_payload: emr::qr_payload(ORG.parse().unwrap(), TOKEN),
    });
    round_trip(&ParticipantIdentity {
        given_name: Some("A".into()),
        family_name: Some("B".into()),
        sex: Some("female".into()),
        birth_year_month: Some("2023-04".into()),
        guardian_phone: Some("+977 1 0000000".into()),
    });
    round_trip(&CaseShareRequest {
        participant_id: PID.into(),
        session_ids: vec!["s1".into()],
        recipient: "Dr B, Kanti Children's Hospital".into(),
        consent_reference: Some("FORM-12".into()),
        consent_recorded_at: Some(TS.into()),
        include_attachments: true,
    });
    round_trip(&share_link());
    round_trip(&CreateShareLinkRequest {
        expires_in_days: Some(3),
        max_downloads: Some(2),
    });
    round_trip(&CreatedShareLink {
        link: share_link(),
        passcode: "ABCDEFGH2345".into(),
    });
    round_trip(&ShareLinkPage {
        items: vec![share_link()],
        next_cursor: None,
    });
    round_trip(&ShareLinkOpenRequest {
        passcode: "ABCDEFGH2345".into(),
    });
    round_trip(&ShareLinkOpenResponse {
        export_id: "x1".into(),
        file_name: "case.zip".into(),
        bytes: 1234,
        expires_at: TS.into(),
        downloads_left: 2,
        download_token: "t".into(),
    });
}

#[test]
fn enums_use_the_database_spelling() {
    for (k, s) in [
        (EncounterKind::ScreeningVisit, "screening_visit"),
        (EncounterKind::FollowUpVisit, "follow_up_visit"),
        (EncounterKind::PhoneCall, "phone_call"),
        (EncounterKind::HomeVisit, "home_visit"),
        (EncounterKind::Other, "other"),
    ] {
        assert_eq!(serde_json::to_value(k).unwrap(), json!(s));
        assert_eq!(k.as_str(), s);
        assert_eq!(EncounterKind::parse(s), Some(k));
    }
    assert_eq!(EncounterKind::ALL.len(), 5);
    assert_eq!(
        serde_json::to_value(EncounterStatus::Closed).unwrap(),
        json!("closed")
    );
    for (k, s) in [
        (AttachmentKind::PaperForm, "paper_form"),
        (AttachmentKind::Referral, "referral"),
        (AttachmentKind::Report, "report"),
        (AttachmentKind::PhotoOfDocument, "photo_of_document"),
        (AttachmentKind::Other, "other"),
    ] {
        assert_eq!(serde_json::to_value(k).unwrap(), json!(s));
        assert_eq!(AttachmentKind::parse(s), Some(k));
    }
    for (m, s) in [
        (AttachmentMediaType::ApplicationPdf, "application/pdf"),
        (AttachmentMediaType::ImageJpeg, "image/jpeg"),
        (AttachmentMediaType::ImagePng, "image/png"),
    ] {
        assert_eq!(serde_json::to_value(m).unwrap(), json!(s));
        assert_eq!(m.as_str(), s);
        assert_eq!(AttachmentMediaType::parse(s), Some(m));
    }
    assert!(serde_json::from_value::<AttachmentMediaType>(json!("image/gif")).is_err());
    assert!(serde_json::from_value::<EncounterKind>(json!("surgery")).is_err());
}

#[test]
fn older_client_bodies_without_optional_fields_parse() {
    let e: CreateEncounterRequest =
        serde_json::from_value(json!({"kind": "other", "occurred_at": TS})).unwrap();
    assert!(e.session_ids.is_empty() && e.summary.is_none() && e.reason.is_none());

    let u: UpdateEncounterRequest = serde_json::from_value(json!({})).unwrap();
    assert!(u.status.is_none() && u.follow_up_done_at.is_none());

    let s: ParticipantSearchRequest = serde_json::from_value(json!({})).unwrap();
    assert!(s.token_prefix.is_none() && s.limit.is_none());

    let p: CreateParticipantRequest = serde_json::from_value(json!({})).unwrap();
    assert!(p.consent_version.is_none());

    let c: CaseShareRequest = serde_json::from_value(json!({
        "participant_id": PID,
        "recipient": "Dr B, district hospital"
    }))
    .unwrap();
    assert!(c.session_ids.is_empty() && !c.include_attachments);
    assert!(c.consent_reference.is_none() && c.consent_recorded_at.is_none());

    let l: CreateShareLinkRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(l.expires_in_days.unwrap_or(emr::SHARE_LINK_DEFAULT_DAYS), 3);

    let a: CreateAttachmentRequest = serde_json::from_value(json!({
        "kind": "report", "file_name": "r.pdf", "media_type": "application/pdf", "byte_size": 10
    }))
    .unwrap();
    assert!(a.encounter_id.is_none());

    // A server that predates `clinician_display_name` / `updated_at` still parses.
    let mut v = serde_json::to_value(encounter()).unwrap();
    let obj = v.as_object_mut().unwrap();
    for k in [
        "clinician_display_name",
        "updated_at",
        "summary",
        "session_ids",
    ] {
        obj.remove(k);
    }
    let back: Encounter = serde_json::from_value(v).unwrap();
    assert!(back.summary.is_none() && back.session_ids.is_empty());
}

#[test]
fn encounter_summary_is_omitted_not_null() {
    let v = serde_json::to_value(Encounter {
        summary: None,
        ..encounter()
    })
    .unwrap();
    assert!(v.get("summary").is_none());
    assert_eq!(v["has_summary"], json!(true));
}

#[test]
fn created_participant_is_a_participant_plus_qr_payload() {
    let created = CreatedParticipant {
        participant: participant(),
        qr_payload: emr::qr_payload(ORG.parse().unwrap(), TOKEN),
    };
    let v = serde_json::to_value(&created).unwrap();
    assert_eq!(v["participant_token"], json!(TOKEN));
    assert_eq!(
        v["qr_payload"],
        json!(format!("asd://participant/{TOKEN}?org={ORG}"))
    );
    // A plain `Participant` reader ignores the extra key.
    let p: Participant = serde_json::from_value(v).unwrap();
    assert_eq!(p, participant());
}

#[test]
fn search_row_flattens_the_participant() {
    let v = serde_json::to_value(ParticipantSearchRow {
        participant: participant(),
        open_encounters_n: 2,
        next_follow_up_at: None,
    })
    .unwrap();
    assert_eq!(v["session_count"], json!(2));
    assert_eq!(v["open_encounters_n"], json!(2));
    assert!(v.get("participant").is_none());
}

#[test]
fn created_share_link_carries_the_passcode_and_share_link_does_not() {
    let v = serde_json::to_value(share_link()).unwrap();
    assert!(v.get("passcode").is_none());
    let schema = schema::api_v1();
    let props = schema["$defs"]["ShareLink"]["properties"]
        .as_object()
        .unwrap();
    assert!(!props.contains_key("passcode"));
    let created = schema["$defs"]["CreatedShareLink"]["properties"]
        .as_object()
        .unwrap();
    assert!(created.contains_key("passcode") && created.contains_key("link"));
}

/// Wave-3 contract, frozen now: no field is renamed on the wire.
#[test]
fn participant_identity_keys_are_frozen() {
    let v = serde_json::to_value(ParticipantIdentity {
        given_name: Some("A".into()),
        family_name: Some("B".into()),
        sex: Some("male".into()),
        birth_year_month: Some("2023-01".into()),
        guardian_phone: Some("1".into()),
    })
    .unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "birth_year_month",
            "family_name",
            "given_name",
            "guardian_phone",
            "sex"
        ]
    );
}

#[test]
fn schema_export_contains_every_def() {
    let schema = schema::api_v1();
    let defs = schema["$defs"].as_object().unwrap();
    for name in [
        "Encounter",
        "EncounterKind",
        "EncounterStatus",
        "CreateEncounterRequest",
        "UpdateEncounterRequest",
        "EncounterPage",
        "LinkSessionRequest",
        "FollowUp",
        "FollowUpPage",
        "Attachment",
        "AttachmentKind",
        "AttachmentMediaType",
        "CreateAttachmentRequest",
        "AttachmentUpload",
        "UploadTarget",
        "ParticipantSearchRequest",
        "ParticipantSearchRow",
        "ParticipantSearchPage",
        "CreateParticipantRequest",
        "CreatedParticipant",
        "ParticipantIdentity",
        "CaseShareRequest",
        "ShareLink",
        "CreatedShareLink",
        "CreateShareLinkRequest",
        "ShareLinkPage",
        "ShareLinkOpenRequest",
        "ShareLinkOpenResponse",
    ] {
        assert!(defs.contains_key(name), "$defs/{name} missing");
    }
    // Optional request fields are not required (older consoles do not send them).
    let required = defs["CaseShareRequest"]["required"].as_array().unwrap();
    let mut required: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    required.sort_unstable();
    assert_eq!(required, ["participant_id", "recipient"]);
}

#[test]
fn rec9_problem_slugs_are_published() {
    let schema = schema::api_v1();
    let slugs: Vec<&str> = schema["$defs"]["ProblemSlug"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    // `totp-reset-forbidden` is WP-B.17's.
    for slug in [
        "encounter-closed",
        "encounter-summary-forbidden",
        "attachment-too-large",
        "attachment-type-not-allowed",
        "digest-mismatch",
        "share-link-expired",
        "share-link-exhausted",
        "share-passcode-invalid",
        "case-consent-missing",
        "identity-disabled",
    ] {
        assert!(slugs.contains(&slug), "{slug} not in ProblemSlug");
        assert!(problem::SLUGS.contains(&slug));
    }
}

// ---- QR payload (REC-1, REC-12) ------------------------------------------------------------

#[test]
fn qr_payload_has_the_rec1_shape() {
    let org: Uuid = ORG.parse().unwrap();
    assert_eq!(
        emr::qr_payload(org, TOKEN),
        format!("asd://participant/{TOKEN}?org={ORG}")
    );
    assert_eq!(
        emr::parse_qr_payload(&emr::qr_payload(org, TOKEN)),
        Ok((org, TOKEN.to_string()))
    );
}

/// SplitMix64: a dependency-free generator for the property test.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[test]
fn qr_payload_round_trips_for_random_orgs_and_tokens() {
    let mut rng = Rng(0x00C0_FFEE);
    for _ in 0..2_000 {
        let org = Uuid::from_u128(u128::from(rng.next()) << 64 | u128::from(rng.next()));
        let hex: String = (0..20)
            .map(|_| char::from(b"0123456789abcdef"[(rng.next() % 16) as usize]))
            .collect();
        let token = format!("child-{hex}");
        assert!(emr::is_participant_token(&token));
        let payload = emr::qr_payload(org, &token);
        assert_eq!(
            emr::parse_qr_payload(&payload),
            Ok((org, token)),
            "{payload}"
        );
    }
}

#[test]
fn mint_participant_token_matches_rec12() {
    let token = emr::participant_token_from_uuid(Uuid::from_u128(
        0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
    ));
    assert_eq!(token, "child-0123456789abcdef0123");
    assert!(emr::is_participant_token(&token));
}

#[test]
fn qr_payload_rejects() {
    use QrParseError as E;
    let ok = format!("asd://participant/{TOKEN}?org={ORG}");
    assert!(emr::parse_qr_payload(&ok).is_ok());
    let cases: Vec<(String, QrParseError)> = vec![
        (format!("https://participant/{TOKEN}?org={ORG}"), E::Scheme),
        (format!("asd:participant/{TOKEN}?org={ORG}"), E::Scheme),
        (format!("asd://patient/{TOKEN}?org={ORG}"), E::Host),
        (format!("asd://participant?org={ORG}"), E::Host),
        (format!("asd://participant/child-0123?org={ORG}"), E::Token),
        (
            format!("asd://participant/child-0123456789ABCDEF0123?org={ORG}"),
            E::Token,
        ),
        (
            format!("asd://participant/kid-0123456789abcdef0123?org={ORG}"),
            E::Token,
        ),
        (format!("asd://participant/{TOKEN}/x?org={ORG}"), E::Token),
        (format!("asd://participant/{TOKEN}"), E::MissingOrg),
        (format!("asd://participant/{TOKEN}?site=1"), E::MissingOrg),
        (format!("asd://participant/{TOKEN}?org=not-a-uuid"), E::Org),
        (
            format!("asd://participant/{TOKEN}?org={}", ORG.replace('-', "")),
            E::Org,
        ),
        (
            format!("asd://participant/{TOKEN}?org={ORG}&org={ORG}"),
            E::DuplicateOrg,
        ),
        (
            format!("asd://participant/{TOKEN}?org={ORG}#x"),
            E::Fragment,
        ),
        (String::new(), E::Scheme),
    ];
    for (input, want) in cases {
        assert_eq!(emr::parse_qr_payload(&input), Err(want), "{input:?}");
    }
}

#[test]
fn qr_payload_tolerates_case_in_scheme_and_host_and_unknown_params() {
    let org: Uuid = ORG.parse().unwrap();
    let upper = format!("ASD://Participant/{TOKEN}?v=1&org={}", ORG.to_uppercase());
    assert_eq!(emr::parse_qr_payload(&upper), Ok((org, TOKEN.to_string())));
}

#[test]
fn limits_match_the_spec() {
    assert_eq!(emr::MAX_ATTACHMENT_BYTES, 10 * 1024 * 1024);
    assert_eq!(emr::MAX_ENCOUNTER_SUMMARY_CHARS, 8_000);
    assert_eq!(emr::MAX_ENCOUNTER_REASON_CHARS, 200);
    assert_eq!(emr::SHARE_LINK_MAX_DAYS, 7);
    assert_eq!(emr::SHARE_PASSCODE_LEN, 12);
    assert!(!emr::SHARE_PASSCODE_ALPHABET.contains(&b'I'));
    assert!(!emr::SHARE_PASSCODE_ALPHABET.contains(&b'O'));
    assert!(!emr::SHARE_PASSCODE_ALPHABET.contains(&b'0'));
    assert!(!emr::SHARE_PASSCODE_ALPHABET.contains(&b'1'));
    assert_eq!(emr::SHARE_PASSCODE_ALPHABET.len(), 32);
    let _: Value = json!(emr::SHARE_LINK_MAX_DOWNLOADS);
}

#[test]
fn file_names_are_sanitised() {
    assert_eq!(
        emr::sanitize_file_name("Referral (Dr A).pdf"),
        "Referral__Dr_A_.pdf"
    );
    assert_eq!(
        emr::sanitize_file_name("../../etc/passwd"),
        "_.._etc_passwd"
    );
    assert_eq!(emr::sanitize_file_name(".hidden"), "hidden");
    assert_eq!(emr::sanitize_file_name("नाम.png"), "___.png");
    assert_eq!(emr::sanitize_file_name(""), "file");
    assert_eq!(emr::sanitize_file_name(&"a".repeat(300)).len(), 120);
}

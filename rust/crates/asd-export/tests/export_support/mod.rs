//! Shared fixtures for the ADR-0008 tests (`deid`, `model_roundtrip`, `policy`).
//!
//! The session comes from the iOS golden ingest body
//! `asd-core/tests/fixtures/expected-session-v1.6.json`. The backend builds `SessionDetail`
//! and `ClipScore` from the stored rows (`read/sessions.rs`, `read/clips.rs`); this fixture
//! maps the ingest body the simple way, because it only needs realistic values.
#![allow(dead_code)] // each test binary uses a different subset

use std::path::PathBuf;

use asd_contract::api_v1::{
    ClipScore, MetricRow, SessionAttentionFlag, SessionDetail, SessionSummary,
};
use asd_contract::session_v1_6::SessionIngestRequest;
use asd_core::types::PerVideoScoreRow;
use asd_export::model::{
    ClinicalReviewSummary, Disposition, ExportRecord, LetterKind, LetterText, Mrn, NoteText,
    OrgFacts, Paper, PatientRef, ReviewState, Signature, SignatureRole,
};
use asd_export::policy::ConsentFacts;

pub const PARTICIPANT_ID: &str = "7d0c3f52-1b8e-4f6a-9c2d-5e4b3a2f1c0d";
pub const DEVICE_ID: &str = "c2a1f0e9-8d7c-4b6a-a5f4-e3d2c1b0a9f8";

pub fn golden_ingest() -> SessionIngestRequest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../asd-core/tests/fixtures/expected-session-v1.6.json");
    let text = std::fs::read_to_string(&path).expect("read the iOS golden session fixture");
    serde_json::from_str(&text).expect("fixture decodes as SessionIngestRequest")
}

fn metric_row(row: &PerVideoScoreRow) -> MetricRow {
    MetricRow {
        segment_id: row.segment_id.clone(),
        metric: row.biomarker.clone(),
        target_aoi: row.target_aoi.clone(),
        direction: row.direction,
        expected: row.expected,
        acceptable_low: row.acceptable_low,
        acceptable_high: row.acceptable_high,
        recorded: row.observed,
        deviation: row.expected.map(|e| row.observed - e),
        status: row.status,
        score: row.score,
        imputed: row.imputed,
        source: row.source.clone(),
    }
}

/// `SessionDetail` + clips for the golden session, as the backend would read them.
pub fn golden_session() -> (SessionDetail, Vec<ClipScore>) {
    let req = golden_ingest();
    let raw = &req.raw_payload;
    let detail = SessionDetail {
        summary: SessionSummary {
            id: req.session_id.clone(),
            participant_id: PARTICIPANT_ID.into(),
            participant_token: req.participant_token.clone(),
            started_at: raw.started_at.clone(),
            duration_sec: req.duration_sec,
            battery: req.battery.clone().flatten(),
            capture_source: Some(req.capture_source),
            quality_verdict: "optimal".into(),
            attention_flag: SessionAttentionFlag::Yellow,
            composite_score: raw.per_video_composite,
            composite_confidence: Some(1.0),
            bundle_version: Some("2026.10.1".into()),
            device_id: Some(DEVICE_ID.into()),
            reviewed: Some(false),
            tracking_verdict: None,
            tracking_accuracy_deg: None,
        },
        org_id: "5f1e2d3c-4b5a-4968-8776-655443322110".into(),
        age_pack: Some("24-36m".into()),
        lang: Some("ne".into()),
        schema_version: Some(req.schema_version.clone()),
        valid_frame_ratio: Some(raw.valid_frame_ratio),
        calibrated_distance_cm: req.calibrated_distance_cm,
        social_preference: Some(req.social_preference_score),
        eye_dwell: req.eye_dwell_ratio,
        mouth_dwell: req.mouth_dwell_ratio,
        first_eye_latency_ms: req.first_eye_latency_ms,
        affect: None,
        report_pdf_available: Some(false),
        tracking_quality: None,
    };
    let clips = req
        .per_video
        .iter()
        .flatten()
        .map(|v| ClipScore {
            clip_id: v.clip_id.clone(),
            order: v.order,
            status: v.status,
            score: Some(v.score),
            weight: Some(v.weight),
            dwell_social: v.dwell_social,
            dwell_geometric: v.dwell_geometric,
            dwell_eye: v.dwell_eye,
            dwell_mouth: v.dwell_mouth,
            orienting_latency_ms: v.orienting_latency_ms,
            gaze_cue_accuracy: None,
            fixation_count: v.fixation_count,
            mean_fixation_ms: v.mean_fixation_ms,
            metric: None,
            expected: None,
            recorded: None,
            deviation: None,
            baseline_ref: None,
            rows: v.details.rows.iter().map(metric_row).collect(),
        })
        .collect();
    (detail, clips)
}

pub fn org() -> OrgFacts {
    OrgFacts {
        name: "Kathmandu Child Clinic".into(),
        timezone: "Asia/Kathmandu".into(),
        paper: Paper::A4,
        letterhead: vec!["Maharajgunj, Kathmandu".into()],
    }
}

/// The golden session as an identified record with no clinical review.
pub fn golden_record() -> ExportRecord {
    let (session, clips) = golden_session();
    let patient = PatientRef {
        participant_id: PARTICIPANT_ID.into(),
        participant_token: session.summary.participant_token.clone(),
        mrn: None,
        age_bracket_at_enrol: Some("24-36m".into()),
    };
    let consent = ConsentFacts {
        withdrawn: false,
        research_consented: Some(true),
    };
    ExportRecord::new(org(), patient, consent, session, clips)
}

/// Sentinel values in every field the de-identification list names (XC-4). None of them
/// may appear anywhere in a de-identified record.
pub mod sentinel {
    pub const MRN: &str = "MRN-SENTINEL-4711";
    pub const AUTHORITY: &str = "TUTH-SENTINEL";
    pub const IMPRESSION: &str = "IMPRESSION-SENTINEL text";
    pub const RECOMMENDATIONS: &str = "RECOMMENDATIONS-SENTINEL text";
    pub const NOTE: &str = "NOTE-SENTINEL body";
    pub const NOTE_AUTHOR: &str = "NOTE-AUTHOR-SENTINEL";
    pub const LETTER_BODY: &str = "LETTER-BODY-SENTINEL";
    pub const LETTER_RECIPIENT: &str = "LETTER-RECIPIENT-SENTINEL";
    pub const SIGNER: &str = "Dr. SIGNER-SENTINEL";
    pub const COSIGNER: &str = "Dr. COSIGNER-SENTINEL";
    pub const REGISTRATION: &str = "NMC-SENTINEL-12345";
    pub const SIGNER_ID: &str = "signer-id-sentinel";
    pub const CONTENT_HASH: &str =
        "c0ffee0000000000000000000000000000000000000000000000000000c0ffee";
}

/// A fully populated identified record: MRN, a signed and cosigned review with impression
/// and recommendations, a note and a letter.
pub fn full_record() -> ExportRecord {
    let mut r = golden_record();
    r.patient.mrn = Some(Mrn {
        value: sentinel::MRN.into(),
        assigning_authority: sentinel::AUTHORITY.into(),
    });
    r.review = Some(ClinicalReviewSummary {
        state: ReviewState::Final,
        version_no: 2,
        amended: true,
        disposition: Some(Disposition::Monitor),
        signed_at: Some("2026-10-02T08:30:00Z".into()),
        content_hash: Some(sentinel::CONTENT_HASH.into()),
        signatures: vec![
            Signature {
                role: SignatureRole::Sign,
                user_id: Some(sentinel::SIGNER_ID.into()),
                display_name: Some(sentinel::SIGNER.into()),
                professional_registration: Some(sentinel::REGISTRATION.into()),
                signed_at: "2026-10-02T08:00:00Z".into(),
            },
            Signature {
                role: SignatureRole::Cosign,
                user_id: Some("cosigner-id-sentinel".into()),
                display_name: Some(sentinel::COSIGNER.into()),
                professional_registration: None,
                signed_at: "2026-10-02T08:30:00Z".into(),
            },
        ],
        impression: Some(sentinel::IMPRESSION.into()),
        recommendations: Some(sentinel::RECOMMENDATIONS.into()),
    });
    r.notes = vec![NoteText {
        created_at: "2026-10-03T11:00:00Z".into(),
        author_display_name: Some(sentinel::NOTE_AUTHOR.into()),
        body: sentinel::NOTE.into(),
    }];
    r.letters = vec![LetterText {
        kind: LetterKind::Referral,
        recipient: sentinel::LETTER_RECIPIENT.into(),
        body: sentinel::LETTER_BODY.into(),
        finalized_at: Some("2026-10-04T12:00:00Z".into()),
    }];
    r
}

/// `full_record` for another session of the same participant, `days` later.
pub fn later_session(days: i64, session_id: &str) -> ExportRecord {
    let mut r = full_record();
    let start = chrono::DateTime::parse_from_rfc3339(&r.session.summary.started_at).unwrap()
        + chrono::Duration::days(days)
        + chrono::Duration::hours(1);
    r.session.summary.started_at = start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    r.session.summary.id = session_id.into();
    r
}

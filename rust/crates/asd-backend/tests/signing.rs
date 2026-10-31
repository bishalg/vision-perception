//! owner: WP-C.10
//!
//! SIGN-4..10: sign, cosign and amend. The signature is bound to the JCS content hash of
//! what the signer saw; step-up re-authentication (password, TOTP when enrolled); cosign
//! policy from org settings and a different cosigner; immutable signed versions; amend is a
//! new version; `interop::on_review_finalized` once per finalisation; one signature under
//! concurrent attempts. Needs `DATABASE_URL`; skipped otherwise.

mod clinical_support;
mod common;

use asd_backend::clinical::reauth::totp_code;
use asd_contract::api_v1::{MetricRow, SessionAttentionFlag};
use asd_contract::clinical::{
    content_hash_hex, ClinicalReviewSnapshot, Disposition, SnapshotClip, SnapshotMeasurements,
    SIGNATURE_MEANING,
};
use clinical_support::*;
use common::auth::{slug, PASSWORD};
use http::StatusCode;
use serde_json::{json, Value};
use sqlx::postgres::PgConnectOptions;
use sqlx::{Connection, PgConnection};

const TOTP_SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn review_of(ctx: &Ctx, id: &str) -> Value {
    let (status, _, body) = get_as(
        ctx,
        &ctx.clin_a.token,
        &format!("/v1/clinical-reviews/{id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn state_of(ctx: &Ctx, id: &str) -> (String, i64) {
    sqlx::query_as("SELECT state, row_version::bigint FROM clinical_reviews WHERE id = $1::uuid")
        .bind(id)
        .fetch_one(ctx.pool())
        .await
        .unwrap()
}

/// The snapshot the server must hash for `draft_ready`'s content (SIGN-4), built here from
/// the fixture values, independently of the server code.
fn expected_snapshot(
    ctx: &Ctx,
    session_id: &str,
    flag: SessionAttentionFlag,
) -> ClinicalReviewSnapshot {
    let row: MetricRow = serde_json::from_value(json!({
        "segment_id": "seg-1", "metric": "eye_dwell_ratio", "target_aoi": null,
        "direction": "higherBetter", "expected": 0.5, "acceptable_low": 0.3,
        "acceptable_high": 0.7, "recorded": 0.42, "deviation": 0.42 - 0.5,
        "status": "monitor", "score": 0.71, "imputed": false, "source": "norm"
    }))
    .unwrap();
    ClinicalReviewSnapshot {
        session_id: session_id.into(),
        org_id: ctx.org.to_string(),
        bundle_version: Some("b-2026.10".into()),
        schema_version: Some("1.6".into()),
        measurements: SnapshotMeasurements {
            started_at: "2026-10-01T09:00:00.250Z".into(),
            duration_sec: 31.5,
            quality_verdict: "optimal".into(),
            attention_flag: flag,
            composite_score: Some(0.37),
            composite_confidence: None,
            valid_frame_ratio: Some(0.93),
            calibrated_distance_cm: None,
            social_preference: Some(0.61),
            eye_dwell: Some(0.42),
            mouth_dwell: None,
            first_eye_latency_ms: None,
        },
        clips: vec![
            SnapshotClip {
                clip_id: "clip-a".into(),
                order: 1,
                rows: vec![],
            },
            SnapshotClip {
                clip_id: "clip-b".into(),
                order: 2,
                rows: vec![row],
            },
        ],
        disposition: Some(Disposition::Monitor),
        impression: Some("Reduced eye dwell in clip 2.".into()),
        recommendations: Some("Recapture in 3 months.".into()),
        version_no: 1,
    }
}

#[tokio::test]
async fn sign_binds_the_content_hash_and_finalises_once() {
    observe_finalized();
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    let id = review["id"].as_str().unwrap();
    let expected = content_hash_hex(&expected_snapshot(&ctx, "S-1", SessionAttentionFlag::Green));
    assert_eq!(
        review["content_hash"],
        json!(expected),
        "SIGN-4 hash of what the signer saw"
    );

    let (status, _, signed) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["state"], "final");
    assert_eq!(signed["row_version"], 3);
    assert_eq!(signed["current"]["status"], "signed");
    assert!(signed["current"]["signed_at"].is_string());
    let sigs = signed["signatures"].as_array().unwrap();
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0]["role"], "sign");
    assert_eq!(sigs[0]["version_no"], 1);
    assert_eq!(sigs[0]["signer_id"], json!(ctx.clin_a.id.to_string()));
    assert_eq!(sigs[0]["signer_display_name"], "Dr A Sharma");
    assert_eq!(sigs[0]["signer_registration"], "NMC 12345");
    assert_eq!(sigs[0]["meaning"], SIGNATURE_MEANING);
    assert_eq!(sigs[0]["content_hash"], json!(expected));
    assert_eq!(finalized_calls(id), 1, "SIGN-10");

    let rows = audit_rows(ctx.pool(), "clinical_review.sign", id).await;
    assert_eq!(rows.len(), 1, "SIGN-9");
    let after = rows[0].1.as_ref().unwrap();
    assert_eq!(after["content_hash"], json!(expected));
    assert_eq!(after["version_no"], 1);
    assert!(!after.to_string().contains("Reduced eye dwell"));

    // Signing again: the review is final.
    let (status, _, body) = sign(&ctx, &ctx.clin_a.token, &signed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(finalized_calls(id), 1);
    ctx.cleanup().await;
}

#[tokio::test]
async fn stale_hash_is_content_changed() {
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    let id = review["id"].as_str().unwrap();
    // Another tab saves after this one displayed the hash.
    let newer = patched(
        &ctx,
        &ctx.clin_a.token,
        &review,
        json!({"impression": "Edited elsewhere."}),
    )
    .await;
    let (status, _, body) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(slug(&body), "content-changed");
    assert_eq!(state_of(&ctx, id).await, ("in_progress".into(), 3));
    assert_eq!(signature_count(ctx.pool(), id).await, 0);
    // Malformed hashes are 400.
    let (status, _, _) = post_action(
        &ctx,
        &ctx.clin_a.token,
        id,
        "sign",
        &sign_body("abc", PASSWORD, None),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = sign(&ctx, &ctx.clin_a.token, &newer).await;
    assert_eq!(status, StatusCode::OK);
    ctx.cleanup().await;
}

#[tokio::test]
async fn hash_covers_the_stored_clip_rows() {
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    // Trap 8: scored rows never change after ingest. If they did, the hash would drift and
    // the signature must be refused.
    sqlx::query("UPDATE session_clip_scores SET details = $1 WHERE session_id = 'S-1' AND clip_id = 'clip-b'")
        .bind(clip_rows(0.43))
        .execute(ctx.pool())
        .await
        .unwrap();
    let (status, _, body) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(slug(&body), "content-changed");
    ctx.cleanup().await;
}

#[tokio::test]
async fn wrong_password_is_401_audited_and_changes_nothing() {
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    let id = review["id"].as_str().unwrap();
    let hash = review["content_hash"].as_str().unwrap();
    let (status, _, body) = post_action(
        &ctx,
        &ctx.clin_a.token,
        id,
        "sign",
        &sign_body(hash, "wrong password!!", None),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "reauthentication-failed");
    assert_eq!(state_of(&ctx, id).await, ("in_progress".into(), 2));
    assert_eq!(signature_count(ctx.pool(), id).await, 0);
    let rows = audit_rows(ctx.pool(), "clinical_review.sign_failed", id).await;
    assert_eq!(rows.len(), 1);
    assert!(!rows[0]
        .1
        .as_ref()
        .map(Value::to_string)
        .unwrap_or_default()
        .contains("wrong password"));
    assert!(audit_rows(ctx.pool(), "clinical_review.sign", id)
        .await
        .is_empty());

    // meaning_ack must be true.
    let mut body = sign_body(hash, PASSWORD, None);
    body["meaning_ack"] = json!(false);
    let (status, _, _) = post_action(&ctx, &ctx.clin_a.token, id, "sign", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    ctx.cleanup().await;
}

#[tokio::test]
async fn step_up_is_rate_limited() {
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    let id = review["id"].as_str().unwrap();
    let hash = review["content_hash"].as_str().unwrap();
    let mut statuses = Vec::new();
    for _ in 0..11 {
        let (status, _, _) = post_action(
            &ctx,
            &ctx.clin_a.token,
            id,
            "sign",
            &sign_body(hash, "nope nope nope", None),
        )
        .await;
        statuses.push(status);
    }
    assert!(
        statuses[..10]
            .iter()
            .all(|s| *s == StatusCode::UNAUTHORIZED),
        "{statuses:?}"
    );
    assert_eq!(statuses[10], StatusCode::TOO_MANY_REQUESTS);
    ctx.cleanup().await;
}

#[tokio::test]
async fn totp_required_when_enrolled() {
    let Some(ctx) = setup().await else { return };
    sqlx::query("UPDATE users SET totp_secret = $2 WHERE id = $1")
        .bind(ctx.clin_a.id)
        .bind(TOTP_SECRET)
        .execute(ctx.pool())
        .await
        .unwrap();
    let review = draft_ready(&ctx, "S-1", "green").await;
    let id = review["id"].as_str().unwrap();
    let hash = review["content_hash"].as_str().unwrap();

    let (status, _, body) = post_action(
        &ctx,
        &ctx.clin_a.token,
        id,
        "sign",
        &sign_body(hash, PASSWORD, None),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no code: {body}");
    assert_eq!(slug(&body), "reauthentication-failed");
    let current = totp_code(TOTP_SECRET, now_unix()).unwrap();
    let wrong = format!("{:06}", (current.parse::<u32>().unwrap() + 1) % 1_000_000);
    let (status, _, _) = post_action(
        &ctx,
        &ctx.clin_a.token,
        id,
        "sign",
        &sign_body(hash, PASSWORD, Some(&wrong)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "wrong code");
    let (status, _, _) = post_action(
        &ctx,
        &ctx.clin_a.token,
        id,
        "sign",
        &sign_body(hash, "bad password!", Some(&current)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "right code, wrong password"
    );
    assert_eq!(
        audit_rows(ctx.pool(), "clinical_review.sign_failed", id)
            .await
            .len(),
        3
    );
    assert_eq!(state_of(&ctx, id).await.0, "in_progress");

    let current = totp_code(TOTP_SECRET, now_unix()).unwrap();
    let (status, _, body) = post_action(
        &ctx,
        &ctx.clin_a.token,
        id,
        "sign",
        &sign_body(hash, PASSWORD, Some(&current)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"], "final");
    ctx.cleanup().await;
}

#[test]
fn totp_matches_rfc_6238() {
    // RFC 6238 appendix B, SHA-1 secret "12345678901234567890", truncated to 6 digits.
    let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
    for (t, code) in [
        (59, "287082"),
        (1_111_111_109, "081804"),
        (1_234_567_890, "005924"),
        (2_000_000_000, "279037"),
    ] {
        assert_eq!(totp_code(secret, t).as_deref(), Some(code), "T={t}");
    }
    assert_eq!(totp_code("not base32!", 59), None);
}

#[tokio::test]
async fn signer_profile_must_be_complete() {
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    set_profile(ctx.pool(), ctx.clin_a.id, None, None).await;
    let (status, _, body) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(slug(&body), "signer-profile-incomplete");
    ctx.cleanup().await;
}

#[tokio::test]
async fn disposition_is_required_to_sign() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "green").await;
    let review = claimed(&ctx, &ctx.clin_a.token, "S-1").await;
    let saved = patched(
        &ctx,
        &ctx.clin_a.token,
        &review,
        json!({"impression": "No disposition yet."}),
    )
    .await;
    let (status, _, body) = sign(&ctx, &ctx.clin_a.token, &saved).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // An unsaved (open) review cannot be signed either.
    session(ctx.pool(), ctx.org, "S-2", "green").await;
    let open = claimed(&ctx, &ctx.clin_a.token, "S-2").await;
    let (status, _, _) = sign(&ctx, &ctx.clin_a.token, &open).await;
    assert_eq!(status, StatusCode::CONFLICT);
    ctx.cleanup().await;
}

#[tokio::test]
async fn sign_roles() {
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    for (who, token) in [
        ("owner", &ctx.owner.token),
        ("admin", &ctx.admin.token),
        ("reviewer", &ctx.reviewer.token),
        ("superadmin", &ctx.superadmin.token),
    ] {
        for action in ["sign", "cosign"] {
            let body = sign_body(review["content_hash"].as_str().unwrap(), PASSWORD, None);
            let (status, _, _) =
                post_action(&ctx, token, review["id"].as_str().unwrap(), action, &body).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{who} {action}");
        }
        let (status, _, _) = amend(&ctx, token, &review, "a reason that is long enough").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who} amend");
    }
    let (status, _, _) = sign(&ctx, &ctx.other_clin.token, &review).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another org");
    ctx.cleanup().await;
}

// ---- cosign (SIGN-6) ------------------------------------------------------------------------

#[tokio::test]
async fn cosign_policy_red_flag_applies_to_red_sessions_only() {
    let Some(ctx) = setup().await else { return };
    set_cosign_policy(ctx.pool(), ctx.org, "red_flag").await;
    let red = draft_ready(&ctx, "S-RED", "red").await;
    let green = draft_ready(&ctx, "S-GREEN", "green").await;
    let (_, _, r) = sign(&ctx, &ctx.clin_a.token, &red).await;
    assert_eq!(r["state"], "awaiting_cosign", "{r}");
    assert_eq!(r["cosign_required"], true);
    let (_, _, g) = sign(&ctx, &ctx.clin_a.token, &green).await;
    assert_eq!(g["state"], "final", "{g}");
    assert_eq!(g["cosign_required"], false);
    ctx.cleanup().await;
}

#[tokio::test]
async fn cosign_policy_always_and_none() {
    let Some(ctx) = setup().await else { return };
    let none = draft_ready(&ctx, "S-NONE", "red").await;
    let (_, _, n) = sign(&ctx, &ctx.clin_a.token, &none).await;
    assert_eq!(n["state"], "final", "default policy is none: {n}");
    set_cosign_policy(ctx.pool(), ctx.org, "always").await;
    let always = draft_ready(&ctx, "S-ALWAYS", "green").await;
    let (_, _, a) = sign(&ctx, &ctx.clin_a.token, &always).await;
    assert_eq!(a["state"], "awaiting_cosign", "{a}");
    ctx.cleanup().await;
}

#[tokio::test]
async fn cosigner_must_differ_and_cosign_finalises() {
    observe_finalized();
    let Some(ctx) = setup().await else { return };
    set_cosign_policy(ctx.pool(), ctx.org, "always").await;
    let review = draft_ready(&ctx, "S-1", "red").await;
    let id = review["id"].as_str().unwrap();
    let (status, _, signed) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["state"], "awaiting_cosign");
    assert_eq!(finalized_calls(id), 0, "not final yet");

    let (status, _, body) = cosign(&ctx, &ctx.clin_a.token, &signed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(slug(&body), "cosigner-must-differ");

    // Wrong hash and wrong password are refused for the cosigner too.
    let (status, _, body) = post_action(
        &ctx,
        &ctx.clin_b.token,
        id,
        "cosign",
        &sign_body(&"0".repeat(64), PASSWORD, None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(slug(&body), "content-changed");
    let hash = signed["content_hash"].as_str().unwrap();
    let (status, _, _) = post_action(
        &ctx,
        &ctx.clin_b.token,
        id,
        "cosign",
        &sign_body(hash, "not the password", None),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        audit_rows(ctx.pool(), "clinical_review.cosign_failed", id)
            .await
            .len(),
        1
    );

    let (status, _, done) = cosign(&ctx, &ctx.clin_b.token, &signed).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["state"], "final");
    let sigs = done["signatures"].as_array().unwrap();
    assert_eq!(sigs.len(), 2);
    assert_eq!(sigs[1]["role"], "cosign");
    assert_eq!(sigs[1]["signer_display_name"], "Dr B Thapa");
    assert_eq!(sigs[1]["signer_registration"], Value::Null);
    assert_eq!(sigs[1]["content_hash"], json!(hash));
    assert_eq!(finalized_calls(id), 1);
    assert_eq!(
        audit_rows(ctx.pool(), "clinical_review.cosign", id)
            .await
            .len(),
        1
    );

    let (status, _, _) = cosign(&ctx, &ctx.clin_b.token, &done).await;
    assert_eq!(status, StatusCode::CONFLICT, "already final");
    assert_eq!(finalized_calls(id), 1);

    // The DB refuses a self-cosign even when the handler is bypassed.
    let err = sqlx::query(
        "INSERT INTO clinical_signatures (org_id, review_id, version_id, signer_id, role, meaning, \
           content_hash, signer_display_name, primary_signer_id) \
         SELECT org_id, review_id, version_id, signer_id, 'cosign', meaning, content_hash, \
           signer_display_name, signer_id FROM clinical_signatures \
         WHERE review_id = $1::uuid AND role = 'sign'",
    )
    .bind(id)
    .execute(ctx.pool())
    .await;
    assert!(err.is_err());
    ctx.cleanup().await;
}

// ---- amend (SIGN-7, SIGN-8) -----------------------------------------------------------------

type VersionBytes = (
    i32,
    String,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<String>,
    Option<String>,
);

async fn version_bytes(ctx: &Ctx, id: &str, version_no: i32) -> VersionBytes {
    sqlx::query_as(
        "SELECT version_no, status, impression_ct, recommendations_ct, content_hash, disposition, \
           signed_at::text FROM clinical_review_versions WHERE review_id = $1::uuid AND version_no = $2",
    )
    .bind(id)
    .bind(version_no)
    .fetch_one(ctx.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn amend_is_a_new_version_and_signed_bytes_never_change() {
    observe_finalized();
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    let id = review["id"].as_str().unwrap();
    let (_, _, signed) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(signed["state"], "final");
    let v1 = version_bytes(&ctx, id, 1).await;
    assert_eq!(v1.1, "signed");

    let (status, _, body) = amend(&ctx, &ctx.clin_a.token, &signed, "too short").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "reason < 10 chars: {body}");
    let (status, _, body) = amend(&ctx, &ctx.clin_a.token, &signed, &"x".repeat(501)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "reason > 500 chars: {body}"
    );

    let (status, _, amending) = amend(
        &ctx,
        &ctx.clin_a.token,
        &signed,
        "Clip 2 was misread; correcting.",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{amending}");
    assert_eq!(amending["state"], "amending");
    assert_eq!(amending["current"]["version_no"], 2);
    assert_eq!(amending["current"]["status"], "draft");
    assert_eq!(
        amending["current"]["impression"], "Reduced eye dwell in clip 2.",
        "content copied"
    );
    assert_eq!(amending["current"]["disposition"], "monitor");
    assert_eq!(
        amending["current"]["amend_reason"],
        "Clip 2 was misread; correcting."
    );
    assert_ne!(
        amending["content_hash"], signed["content_hash"],
        "version_no is hashed"
    );
    assert_eq!(
        version_bytes(&ctx, id, 1).await,
        v1,
        "amend leaves v1 untouched"
    );
    let rows = audit_rows(ctx.pool(), "clinical_review.amend", id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1.as_ref().unwrap()["version_no"], 2);
    assert!(!rows[0].1.as_ref().unwrap().to_string().contains("misread"));

    let edited = patched(
        &ctx,
        &ctx.clin_a.token,
        &amending,
        json!({"impression": "Typical eye dwell."}),
    )
    .await;
    assert_eq!(edited["state"], "amending");
    let (status, _, resigned) = sign(&ctx, &ctx.clin_a.token, &edited).await;
    assert_eq!(status, StatusCode::OK, "{resigned}");
    assert_eq!(resigned["state"], "final");
    assert_eq!(resigned["current"]["version_no"], 2);
    assert_eq!(resigned["current"]["impression"], "Typical eye dwell.");
    assert_eq!(resigned["signatures"].as_array().unwrap().len(), 2);
    assert_eq!(
        finalized_calls(id),
        2,
        "a signed amendment is a finalisation"
    );

    // v1 is superseded with every other byte unchanged.
    let v1_after = version_bytes(&ctx, id, 1).await;
    assert_eq!(v1_after.1, "superseded");
    assert_eq!(
        (
            &v1_after.2,
            &v1_after.3,
            &v1_after.4,
            &v1_after.5,
            &v1_after.6
        ),
        (&v1.2, &v1.3, &v1.4, &v1.5, &v1.6)
    );

    // SIGN-7: the DB refuses edits of signed content and deletion of signatures.
    let upd = sqlx::query("UPDATE clinical_review_versions SET impression_ct = '\\x00' WHERE review_id = $1::uuid AND status = 'signed'")
        .bind(id)
        .execute(ctx.pool())
        .await;
    assert!(upd.is_err());
    let del = sqlx::query("DELETE FROM clinical_signatures WHERE review_id = $1::uuid")
        .bind(id)
        .execute(ctx.pool())
        .await;
    assert!(del.is_err());
    ctx.cleanup().await;
}

#[tokio::test]
async fn amend_while_awaiting_cosign_withdraws_the_cosign() {
    observe_finalized();
    let Some(ctx) = setup().await else { return };
    set_cosign_policy(ctx.pool(), ctx.org, "always").await;
    let review = draft_ready(&ctx, "S-1", "red").await;
    let id = review["id"].as_str().unwrap();
    let (_, _, signed) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(signed["state"], "awaiting_cosign");

    let (status, _, amending) = amend(
        &ctx,
        &ctx.clin_a.token,
        &signed,
        "Wrong clip referenced in impression.",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{amending}");
    assert_eq!(amending["state"], "amending");
    let rows = audit_rows(ctx.pool(), "clinical_review.amend", id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1.as_ref().unwrap()["withdrew_cosign"], true);

    // The withdrawn cosign cannot be given any more.
    let (status, _, body) = cosign(&ctx, &ctx.clin_b.token, &signed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(finalized_calls(id), 0);

    // The amendment goes through signing (and cosign) again.
    let (_, _, resigned) = sign(&ctx, &ctx.clin_a.token, &amending).await;
    assert_eq!(resigned["state"], "awaiting_cosign", "{resigned}");
    let (status, _, done) = cosign(&ctx, &ctx.clin_b.token, &resigned).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["state"], "final");
    assert_eq!(finalized_calls(id), 1);
    ctx.cleanup().await;
}

// ---- concurrency ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_sign_attempts_produce_one_signature() {
    observe_finalized();
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    let id = review["id"].as_str().unwrap().to_owned();
    let body = sign_body(review["content_hash"].as_str().unwrap(), PASSWORD, None);

    // Make the race certain: a separate connection holds the review and draft rows, so
    // the sign transactions that passed every pre-check queue up behind it and are
    // released together. Without the `FOR UPDATE` + re-check they would all write.
    let opts: PgConnectOptions = std::env::var("DATABASE_URL").unwrap().parse().unwrap();
    let mut blocker = PgConnection::connect_with(&opts.database(&ctx.db.name))
        .await
        .unwrap();
    sqlx::query("BEGIN").execute(&mut blocker).await.unwrap();
    sqlx::query("SELECT 1 FROM clinical_reviews WHERE id = $1::uuid FOR UPDATE")
        .bind(&id)
        .execute(&mut blocker)
        .await
        .unwrap();
    sqlx::query("SELECT 1 FROM clinical_review_versions WHERE review_id = $1::uuid FOR UPDATE")
        .bind(&id)
        .execute(&mut blocker)
        .await
        .unwrap();

    // Two signers so the per-user step-up limit (10/min) does not refuse any attempt.
    let mut tasks = Vec::new();
    for i in 0..20 {
        let app = ctx.app.clone();
        let token = if i % 2 == 0 {
            ctx.clin_a.token.clone()
        } else {
            ctx.clin_b.token.clone()
        };
        let req = request(
            http::Method::POST,
            &format!("/v1/clinical-reviews/{id}/sign"),
            Some(&body),
            Some(&token),
            &[],
        );
        tasks.push(tokio::spawn(async move { common::send(app, req).await.0 }));
    }
    // Wait until at least two sign transactions are blocked on the rows.
    let mut waiting = 0;
    for _ in 0..200 {
        waiting = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() \
               AND wait_event_type = 'Lock'",
        )
        .fetch_one(&mut blocker)
        .await
        .unwrap();
        if waiting >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        waiting >= 2,
        "only {waiting} sign transactions reached the lock"
    );
    sqlx::query("ROLLBACK").execute(&mut blocker).await.unwrap();
    blocker.close().await.unwrap();

    let mut ok = 0;
    for t in tasks {
        let status = t.await.unwrap();
        assert!(
            [StatusCode::OK, StatusCode::CONFLICT].contains(&status),
            "{status}"
        );
        ok += usize::from(status == StatusCode::OK);
    }
    assert_eq!(ok, 1);
    assert_eq!(signature_count(ctx.pool(), &id).await, 1);
    assert_eq!(
        audit_rows(ctx.pool(), "clinical_review.sign", &id)
            .await
            .len(),
        1
    );
    assert_eq!(finalized_calls(&id), 1);
    let r = review_of(&ctx, &id).await;
    assert_eq!(r["state"], "final");
    ctx.cleanup().await;
}

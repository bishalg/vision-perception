//! owner: WP-B.3
//!
//! `POST /v1/sessions` (ING-1, ING-2, CON-13..17, CON-21): validation without server
//! defaults, idempotent replay (200 `duplicate:true`) vs body mismatch (409), 20 concurrent
//! identical posts give one row, derived columns, server-side flag (asd-core) or
//! `unreliable`, upload URLs per blob mode.
//!
//! Every request authenticates for real (an enrolled device's `dvc_` bearer), so apart from
//! the no-bearer 401 these tests need Postgres and skip without `DATABASE_URL` (see
//! `common`). Also: AUTH-3 per-device rate limit, auth boundaries, withdrawn consent.

mod common;
mod ingest_support;
mod tracking_support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use asd_backend::blob::{BlobError, BlobStore, MemoryBlobStore, PresignedPut, StoredBlob};
use asd_backend::config::BlobMode;
use asd_contract::session_v1_6::SessionIngestRequest;
use asd_core::flag::{assess, FlagThresholds};
use asd_core::types::PerVideoSessionScore;
use async_trait::async_trait;
use bytes::Bytes;
use common::{send, TestDb};
use http::StatusCode;
use ingest_support::*;
use serde_json::{json, Value};
use sqlx::Row;
use tracking_support::*;
use uuid::Uuid;

/// Posts `body` as `f`'s enrolled device (validation tests need the DB only to
/// authenticate).
async fn post_as(f: &Fixture, body: &Value) -> (StatusCode, Value) {
    let (s, _, b) = send(f.router(), post_session(Some(&f.dev()), body)).await;
    (s, b)
}

fn param_names(body: &Value) -> Vec<String> {
    body["invalid_params"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| p["name"].as_str().unwrap_or("").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

// ---- validation (CON-13, CON-14, CON-21) -------------------------------------------------

#[tokio::test]
async fn without_device_identity_is_401_problem() {
    let (status, headers, body) = send(
        app(common::state_without_db()),
        post_session(None, &golden()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(common::content_type(&headers), "application/problem+json");
    assert_eq!(problem_slug(&body), "unauthorized");
}

#[tokio::test]
async fn missing_required_field_is_400_validation_failed_naming_it() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    for field in [
        "sessionId",
        "validRatio",
        "durationSec",
        "socialPreferenceScore",
        "qualityVerdict",
        "rawPayload",
        "participantToken",
        "captureSource",
    ] {
        let mut body = golden();
        body.as_object_mut().unwrap().remove(field);
        let raw = serde_json::to_vec(&body).unwrap();
        let (status, _, resp) = send(
            f.router(),
            post_session_raw(Some(&f.dev()), Some("SESS-GOLDEN-0001"), Some("1.6"), raw),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {resp}");
        assert_eq!(problem_slug(&resp), "validation-failed", "{field}");
        assert!(
            param_names(&resp).iter().any(|n| n.ends_with(field)),
            "{field} not named in {resp}"
        );
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn out_of_range_ratio_is_rejected_not_clamped() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["perVideo"][0]["dwell_social"] = json!(1.5);
    body["validRatio"] = json!(-0.1);
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert_eq!(problem_slug(&resp), "validation-failed");
    let names = param_names(&resp);
    // The schema wraps optional `perVideo` in `anyOf [array, null]`, so the validator names
    // the nearest enclosing pointer; it must lie on the path to the bad value.
    assert!(
        names
            .iter()
            .any(|n| n.len() > 1 && "/perVideo/0/dwell_social".starts_with(n.as_str())),
        "{names:?}"
    );
    assert!(names.iter().any(|n| n == "/validRatio"), "{names:?}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn malformed_json_is_400_validation_failed() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let (status, _, resp) = send(
        f.router(),
        post_session_raw(
            Some(&f.dev()),
            Some("S"),
            Some("1.6"),
            b"{not json".to_vec(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&resp), "validation-failed");
    f.db.cleanup().await;
}

#[tokio::test]
async fn schema_version_header_must_be_present_equal_and_supported() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let raw = serde_json::to_vec(&golden()).unwrap();
    for header in [None, Some("1.5"), Some("1.7")] {
        let (status, _, resp) = send(
            f.router(),
            post_session_raw(
                Some(&f.dev()),
                Some("SESS-GOLDEN-0001"),
                header,
                raw.clone(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{header:?}");
        assert_eq!(
            problem_slug(&resp),
            "unsupported-schema-version",
            "{header:?}"
        );
    }
    // Header supported but the body says otherwise.
    let mut body = golden();
    body["schemaVersion"] = json!("1.5");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&resp), "unsupported-schema-version");
    f.db.cleanup().await;
}

#[tokio::test]
async fn simulated_capture_is_always_rejected() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["captureSource"] = json!("simulated");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&resp), "capture-source-rejected");
    f.db.cleanup().await;
}

#[tokio::test]
async fn synthetic_capture_is_rejected_unless_allowed() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["captureSource"] = json!("synthetic");
    body["rawPayload"]["captureSource"] = json!("synthetic");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&resp), "capture-source-rejected");
    f.db.cleanup().await;
}

#[tokio::test]
async fn idempotency_key_must_equal_session_id() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let raw = serde_json::to_vec(&golden()).unwrap();
    let (status, _, resp) = send(
        f.router(),
        post_session_raw(Some(&f.dev()), Some("OTHER"), Some("1.6"), raw),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&resp), "validation-failed");
    assert_eq!(param_names(&resp), vec!["X-Idempotency-Key".to_owned()]);
    f.db.cleanup().await;
}

#[tokio::test]
async fn identifiers_must_be_path_and_key_safe() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    for bad in ["a/b", "..", "has space", "", "q?x"] {
        let body = golden_with_id(bad);
        let (status, _, resp) = send(
            f.router(),
            post_session_raw(
                Some(&f.dev()),
                None,
                Some("1.6"),
                serde_json::to_vec(&body).unwrap(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}: {resp}");
        assert_eq!(problem_slug(&resp), "validation-failed", "{bad:?}");
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn raw_payload_must_describe_the_same_session_and_participant() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["rawPayload"]["sessionId"] = json!("SOMETHING-ELSE");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        param_names(&resp).contains(&"/rawPayload/sessionId".to_owned()),
        "{resp}"
    );

    let mut body = golden();
    body["rawPayload"]["participantId"] = json!("child-other");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        param_names(&resp).contains(&"/rawPayload/participantId".to_owned()),
        "{resp}"
    );
    f.db.cleanup().await;
}

#[tokio::test]
async fn duplicate_clip_ids_and_bad_dates_are_rejected() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["perVideo"][1]["clip_id"] = json!("stim_05_geopref_split_10s");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        param_names(&resp).contains(&"/perVideo/1/clip_id".to_owned()),
        "{resp}"
    );

    let mut body = golden();
    body["rawPayload"]["startedAt"] = json!("yesterday");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        param_names(&resp).contains(&"/rawPayload/startedAt".to_owned()),
        "{resp}"
    );
    f.db.cleanup().await;
}

// ---- ingest against Postgres -------------------------------------------------------------

struct Fixture {
    db: TestDb,
    org: Uuid,
    device: Uuid,
    token: String,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let db = TestDb::migrated().await?;
        let org = seed_quiet_org(&db.pool).await;
        let (device, token) = seed_device(&db.pool, org).await;
        Some(Self {
            db,
            org,
            device,
            token,
        })
    }
    /// The device's bearer token.
    fn dev(&self) -> String {
        self.token.clone()
    }
    fn router(&self) -> axum::Router {
        app(state(self.db.pool.clone()))
    }
    async fn count(&self, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.pool)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn golden_payload_is_accepted_with_proxy_upload_urls() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let (status, headers, body) = send(f.router(), post_session(Some(&f.dev()), &golden())).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(common::content_type(&headers).starts_with("application/json"));
    assert_eq!(
        body,
        json!({
            "sessionId": "SESS-GOLDEN-0001",
            "status": "accepted",
            "duplicate": false,
            "frames_upload_url": "/v1/sessions/SESS-GOLDEN-0001/blobs/frames",
            "report_upload_url": "/v1/sessions/SESS-GOLDEN-0001/blobs/report",
        })
    );
    // The body is the contract type (API-3).
    serde_json::from_value::<asd_contract::session_v1_6::SessionIngestResponse>(body).unwrap();
    f.db.cleanup().await;
}

#[tokio::test]
async fn stored_columns_come_from_the_payload_without_defaults() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let payload = golden();
    ingest_ok(&f.router(), &f.dev(), &payload).await;

    let row = sqlx::query(
        "SELECT s.*, p.token AS p_token, p.org_id AS p_org FROM sessions s \
         JOIN participants p ON p.id = s.participant_id WHERE s.id = 'SESS-GOLDEN-0001'",
    )
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    let text = |c: &str| row.get::<Option<String>, _>(c);
    let num = |c: &str| row.get::<Option<f64>, _>(c);
    assert_eq!(row.get::<Uuid, _>("org_id"), f.org);
    assert_eq!(row.get::<Uuid, _>("p_org"), f.org);
    assert_eq!(row.get::<Option<Uuid>, _>("device_id"), Some(f.device));
    assert_eq!(text("participant_token").as_deref(), Some("child-SESS-GOL"));
    assert_eq!(text("p_token").as_deref(), Some("child-SESS-GOL"));
    assert_eq!(text("capture_source").as_deref(), Some("truedepth"));
    assert_eq!(
        row.get::<chrono::DateTime<chrono::Utc>, _>("started_at")
            .to_rfc3339(),
        "2026-10-26T07:33:20+00:00"
    );
    assert_eq!(
        row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("ended_at")
            .unwrap()
            .timestamp_millis(),
        chrono::DateTime::parse_from_rfc3339("2026-10-26T07:33:52.500Z")
            .unwrap()
            .timestamp_millis()
    );
    assert_eq!(num("duration_sec"), Some(32.5));
    // CON-16 / CON-14: explicit null battery stays NULL; no "express_30s" default.
    assert_eq!(text("battery"), None);
    assert_eq!(text("schema_version").as_deref(), Some("1.6"));
    assert_eq!(text("quality_verdict").as_deref(), Some("optimal"));
    assert_eq!(num("valid_frame_ratio"), Some(0.93));
    assert_eq!(num("calibrated_distance_cm"), Some(38.0));
    assert_eq!(num("social_preference"), Some(0.62));
    assert_eq!(num("eye_dwell"), Some(0.33));
    // Unmeasured: absent keys stay NULL (CON-14).
    assert_eq!(num("mouth_dwell"), None);
    assert_eq!(num("first_eye_latency_ms"), None);
    // ING-2.
    assert_eq!(num("composite_score"), Some(0.74));
    assert_eq!(num("composite_confidence"), Some(0.5));
    assert_eq!(text("bundle_version"), None);
    assert_eq!(text("attention_flag").as_deref(), Some("yellow"));
    assert_eq!(text("idempotency_key").as_deref(), Some("SESS-GOLDEN-0001"));
    assert_eq!(row.get::<Value, _>("summary"), payload["rawPayload"]);
    assert_eq!(
        row.get::<Option<Value>, _>("affect").unwrap(),
        json!({
            "flatAffectScore": 0.12,
            "smileReactivityIndex": 0.22,
            "socialSmileIntensity": 0.41,
            "surpriseScoreMean": 0.18
        })
    );
    f.db.cleanup().await;
}

#[tokio::test]
async fn clip_rows_are_stored_from_per_video() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let payload = golden();
    ingest_ok(&f.router(), &f.dev(), &payload).await;
    let rows = sqlx::query(
        "SELECT * FROM session_clip_scores WHERE session_id = 'SESS-GOLDEN-0001' ORDER BY clip_order",
    )
    .fetch_all(&f.db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    let r = &rows[0];
    assert_eq!(r.get::<String, _>("clip_id"), "stim_05_geopref_split_10s");
    assert_eq!(r.get::<i32, _>("clip_order"), 1);
    assert_eq!(r.get::<Option<f64>, _>("dwell_social"), Some(0.62));
    assert_eq!(r.get::<Option<f64>, _>("dwell_geometric"), Some(0.21));
    assert_eq!(r.get::<Option<f64>, _>("dwell_eye"), Some(0.33));
    assert_eq!(r.get::<Option<f64>, _>("dwell_mouth"), None);
    assert_eq!(r.get::<Option<f64>, _>("orienting_latency_ms"), None);
    assert_eq!(r.get::<Option<i32>, _>("fixation_count"), Some(14));
    assert_eq!(r.get::<Option<f64>, _>("mean_fixation_ms"), Some(280.0));
    assert_eq!(
        r.get::<Option<String>, _>("status").as_deref(),
        Some("atypical")
    );
    assert_eq!(r.get::<Option<f64>, _>("score"), Some(0.74));
    assert_eq!(r.get::<Option<f64>, _>("weight"), Some(0.3));
    // Worst row: the atypical (imputed) latency row; its citation is the reference.
    assert_eq!(
        r.get::<Option<String>, _>("baseline_ref").as_deref(),
        Some("provisional")
    );
    assert_eq!(
        r.get::<Option<Value>, _>("details").unwrap(),
        payload["perVideo"][0]["details"]
    );

    let u = &rows[1];
    assert_eq!(u.get::<String, _>("clip_id"), "untagged");
    assert_eq!(u.get::<Option<String>, _>("status"), None);
    assert_eq!(u.get::<Option<f64>, _>("dwell_social"), None);
    assert_eq!(u.get::<Option<String>, _>("baseline_ref"), None);
    f.db.cleanup().await;
}

#[tokio::test]
async fn identical_replay_is_200_duplicate_with_the_original_body() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    let first = ingest_ok(&router, &f.dev(), &golden()).await;

    // Same JSON, different key order and whitespace: still the same body (JCS hash).
    let pretty = serde_json::to_vec_pretty(&golden()).unwrap();
    let (status, _, replay) = send(
        router.clone(),
        post_session_raw(
            Some(&f.dev()),
            Some("SESS-GOLDEN-0001"),
            Some("1.6"),
            pretty,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    let mut expected = first.clone();
    expected["duplicate"] = json!(true);
    assert_eq!(replay, expected);

    assert_eq!(f.count("SELECT count(*) FROM sessions").await, 1);
    assert_eq!(f.count("SELECT count(*) FROM session_clip_scores").await, 2);
    f.db.cleanup().await;
}

#[tokio::test]
async fn same_key_with_a_different_body_is_409_and_changes_nothing() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    ingest_ok(&router, &f.dev(), &golden()).await;
    let mut changed = golden();
    changed["durationSec"] = json!(40.0);
    let (status, headers, body) = send(router, post_session(Some(&f.dev()), &changed)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(common::content_type(&headers), "application/problem+json");
    assert_eq!(problem_slug(&body), "idempotency-key-reused");
    let dur: f64 = sqlx::query_scalar("SELECT duration_sec FROM sessions")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(dur, 32.5);
    f.db.cleanup().await;
}

#[tokio::test]
async fn twenty_concurrent_identical_posts_yield_one_row() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let router = router.clone();
        let dev = f.dev();
        tasks.push(tokio::spawn(async move {
            send(router, post_session(Some(&dev), &golden())).await
        }));
    }
    let mut created = 0;
    let mut replayed = 0;
    for t in tasks {
        let (status, _, body) = t.await.unwrap();
        match status {
            StatusCode::CREATED => created += 1,
            StatusCode::OK => {
                assert_eq!(body["duplicate"], true);
                replayed += 1;
            }
            other => panic!("unexpected {other}: {body}"),
        }
    }
    assert_eq!((created, replayed), (1, 19));
    assert_eq!(f.count("SELECT count(*) FROM sessions").await, 1);
    assert_eq!(f.count("SELECT count(*) FROM participants").await, 1);
    assert_eq!(f.count("SELECT count(*) FROM idempotency_keys").await, 1);
    assert_eq!(f.count("SELECT count(*) FROM session_clip_scores").await, 2);
    f.db.cleanup().await;
}

#[tokio::test]
async fn idempotency_is_scoped_per_org_and_a_taken_session_id_is_409() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    ingest_ok(&router, &f.dev(), &golden()).await;
    let org_b = seed_quiet_org(&f.db.pool).await;
    let (_, dev_b) = seed_device(&f.db.pool, org_b).await;
    let (status, _, body) = send(router, post_session(Some(&dev_b), &golden())).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    // Nothing of org B's attempt survives (one transaction).
    assert_eq!(f.count("SELECT count(*) FROM idempotency_keys").await, 1);
    assert_eq!(f.count("SELECT count(*) FROM participants").await, 1);
    let scope: String = sqlx::query_scalar("SELECT scope FROM idempotency_keys")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(scope, format!("sessions:{}", f.org));
    f.db.cleanup().await;
}

#[tokio::test]
async fn participant_is_upserted_by_org_and_token() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    ingest_ok(&router, &f.dev(), &golden_with_id("S-1")).await;
    ingest_ok(&router, &f.dev(), &golden_with_id("S-2")).await;
    assert_eq!(f.count("SELECT count(*) FROM participants").await, 1);
    assert_eq!(
        f.count("SELECT count(DISTINCT participant_id) FROM sessions")
            .await,
        1
    );
    f.db.cleanup().await;
}

async fn stored_flag(f: &Fixture, id: &str) -> String {
    sqlx::query_scalar("SELECT attention_flag FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(&f.db.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn null_flag_with_low_valid_ratio_is_stored_unreliable() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["attentionFlag"] = Value::Null;
    body["validRatio"] = json!(0.5);
    ingest_ok(&f.router(), &f.dev(), &body).await;
    assert_eq!(stored_flag(&f, "SESS-GOLDEN-0001").await, "unreliable");
    f.db.cleanup().await;
}

#[tokio::test]
async fn null_flag_with_scored_clips_is_computed_by_asd_core() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["attentionFlag"] = Value::Null;
    ingest_ok(&f.router(), &f.dev(), &body).await;

    // The oracle: asd-core over rawPayload.perVideo with the payload composite.
    let typed: SessionIngestRequest = serde_json::from_value(body).unwrap();
    let score = PerVideoSessionScore {
        age_bracket_id: String::new(),
        age_months: 0,
        composite_score: typed.raw_payload.per_video_composite.unwrap(),
        videos: typed.raw_payload.per_video.clone().unwrap(),
        baseline_status: String::new(),
        baseline_schema_version: String::new(),
        scored_at_utc: String::new(),
    };
    let expected = serde_json::to_value(assess(&score, &FlagThresholds::default()).flag).unwrap();
    assert_eq!(expected, "green");
    assert_eq!(stored_flag(&f, "SESS-GOLDEN-0001").await, "green");
    f.db.cleanup().await;
}

#[tokio::test]
async fn null_flag_without_scored_clips_or_per_video_is_unreliable() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();

    let mut no_baseline = golden_with_id("S-NB");
    no_baseline["attentionFlag"] = Value::Null;
    no_baseline["rawPayload"]["perVideo"][0]["hasBaseline"] = json!(false);
    ingest_ok(&router, &f.dev(), &no_baseline).await;
    assert_eq!(stored_flag(&f, "S-NB").await, "unreliable");

    let mut absent = golden_with_id("S-NPV");
    absent.as_object_mut().unwrap().remove("attentionFlag");
    absent["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("perVideo");
    ingest_ok(&router, &f.dev(), &absent).await;
    assert_eq!(stored_flag(&f, "S-NPV").await, "unreliable");
    f.db.cleanup().await;
}

#[tokio::test]
async fn battery_is_the_catalog_protocol_id_and_bundle_version_is_read() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["battery"] = json!("express_30s");
    body["rawPayload"]["bundleVersion"] = json!("2026.10.30-1");
    ingest_ok(&f.router(), &f.dev(), &body).await;
    let (battery, bundle): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT battery, bundle_version FROM sessions")
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert_eq!(battery.as_deref(), Some("express_30s"));
    assert_eq!(bundle.as_deref(), Some("2026.10.30-1"));
    f.db.cleanup().await;
}

#[tokio::test]
async fn synthetic_capture_is_accepted_when_allowed() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    body["captureSource"] = json!("synthetic");
    body["rawPayload"]["captureSource"] = json!("synthetic");
    let st = state_with(
        f.db.pool.clone(),
        Arc::new(MemoryBlobStore::new()),
        config(true),
    );
    ingest_ok(&app(st), &f.dev(), &body).await;
    let cs: String = sqlx::query_scalar("SELECT capture_source FROM sessions")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(cs, "synthetic");
    f.db.cleanup().await;
}

#[tokio::test]
async fn ingest_writes_one_device_audit_row_without_phi() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_ok(&f.router(), &f.dev(), &golden()).await;
    // Registering the device wrote its own row; this is the ingest one.
    let row = sqlx::query("SELECT * FROM audit_log WHERE action = 'session.ingest'")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("actor_type"), "device");
    assert_eq!(
        row.get::<Option<String>, _>("actor_id"),
        Some(f.device.to_string())
    );
    assert_eq!(row.get::<Option<Uuid>, _>("org_id"), Some(f.org));
    assert_eq!(row.get::<String, _>("action"), "session.ingest");
    assert_eq!(
        row.get::<Option<String>, _>("target_id").as_deref(),
        Some("SESS-GOLDEN-0001")
    );
    let after = row
        .get::<Option<Value>, _>("after")
        .unwrap_or(Value::Null)
        .to_string();
    assert!(!after.contains("child-SESS-GOL"), "{after}");
    f.db.cleanup().await;
}

// ---- upload URLs (CON-18) ----------------------------------------------------------------

/// Counts pre-signs and puts the count into each URL, so a fresh URL is observable.
#[derive(Default)]
struct CountingStore {
    inner: MemoryBlobStore,
    presigned: AtomicUsize,
}

#[async_trait]
impl BlobStore for CountingStore {
    async fn presign_put(&self, key: &str, content_type: &str) -> Result<PresignedPut, BlobError> {
        let n = self.presigned.fetch_add(1, Ordering::SeqCst);
        let mut p = self.inner.presign_put(key, content_type).await?;
        p.url = format!("{}&n={n}&ct={content_type}", p.url);
        Ok(p)
    }
    async fn put(&self, key: &str, ct: &str, body: Bytes) -> Result<(), BlobError> {
        self.inner.put(key, ct, body).await
    }
    async fn get(&self, key: &str) -> Result<Option<StoredBlob>, BlobError> {
        self.inner.get(key).await
    }
    async fn ready(&self) -> Result<(), BlobError> {
        Ok(())
    }
}

#[tokio::test]
async fn presigned_mode_returns_absolute_urls_and_replay_gets_fresh_ones() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let store = Arc::new(CountingStore::default());
    let mut cfg = config(false);
    cfg.blob_mode = BlobMode::Presigned;
    let router = app(state_with(f.db.pool.clone(), store.clone(), cfg));

    let first = ingest_ok(&router, &f.dev(), &golden()).await;
    let frames = first["frames_upload_url"].as_str().unwrap();
    let report = first["report_upload_url"].as_str().unwrap();
    let base = format!(
        "https://blobs.memory.test/sessions/{}/child-SESS-GOL/SESS-GOLDEN-0001/frames.jsonl.enc?",
        f.org
    );
    assert!(frames.starts_with(&base), "{frames}");
    assert!(frames.ends_with("ct=application/octet-stream"), "{frames}");
    assert!(
        report.starts_with(&format!(
            "https://blobs.memory.test/reports/{}/child-SESS-GOL/SESS-GOLDEN-0001/report.pdf?",
            f.org
        )),
        "{report}"
    );
    assert!(report.ends_with("ct=application/pdf"), "{report}");

    let (status, _, replay) = send(router, post_session(Some(&f.dev()), &golden())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["duplicate"], true);
    assert_ne!(replay["frames_upload_url"], first["frames_upload_url"]);
    assert_ne!(replay["report_upload_url"], first["report_upload_url"]);
    assert_eq!(store.presigned.load(Ordering::SeqCst), 4);
    f.db.cleanup().await;
}

// ---- auth, rate limit, consent (AUTH-3, AUTH-4) -----------------------------------------

#[tokio::test]
async fn post_sessions_needs_a_live_device_bearer_and_never_touches_another_org() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();

    // No bearer, an unknown dvc_ token, or a console user's JWT: 401 problem, nothing stored.
    let (_, user_token) = seed_user(&f.db.pool, Some(f.org), "owner").await;
    let unknown = format!("dvc_{}", "A".repeat(43));
    for bearer in [None, Some(unknown.as_str()), Some(user_token.as_str())] {
        let (status, headers, body) = send(router.clone(), post_session(bearer, &golden())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{bearer:?}: {body}");
        assert_eq!(common::content_type(&headers), "application/problem+json");
        assert_eq!(problem_slug(&body), "unauthorized");
    }
    assert_eq!(f.count("SELECT count(*) FROM sessions").await, 0);

    // Org A's session; a device of org B can neither replay it into org B, nor upload to
    // it, nor can org B's user read it: 409 (taken id) / 404 / 404.
    ingest_ok(&router, &f.dev(), &golden()).await;
    let org_b = seed_quiet_org(&f.db.pool).await;
    let (_, dev_b) = seed_device(&f.db.pool, org_b).await;
    let (status, _, body) = send(router.clone(), post_session(Some(&dev_b), &golden())).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let put = http::Request::builder()
        .method("PUT")
        .uri("/v1/sessions/SESS-GOLDEN-0001/blobs/frames")
        .header("content-type", "application/octet-stream")
        .header(http::header::AUTHORIZATION, bearer(&dev_b))
        .body(axum::body::Body::from(vec![1u8]))
        .unwrap();
    let (status, _, body) = send(router.clone(), put).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(problem_slug(&body), "not-found");
    let (_, user_b) = seed_user(&f.db.pool, Some(org_b), "owner").await;
    for uri in [
        "/v1/sessions/SESS-GOLDEN-0001".to_owned(),
        format!("/v1/orgs/{}/sessions", f.org),
    ] {
        let (status, _, body) = send(router.clone(), user_get(&user_b, &uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
        assert_eq!(problem_slug(&body), "not-found");
    }
    let owner: Uuid = sqlx::query_scalar("SELECT org_id FROM sessions")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(owner, f.org);

    // A revoked device's token stops working at once.
    sqlx::query("UPDATE devices SET revoked_at = now() WHERE id = $1")
        .bind(f.device)
        .execute(&f.db.pool)
        .await
        .unwrap();
    let (status, _, _) = send(
        router,
        post_session(Some(&f.dev()), &golden_with_id("S-AFTER-REVOKE")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    f.db.cleanup().await;
}

#[tokio::test]
async fn post_sessions_is_limited_to_60_per_minute_per_device() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    // Invalid bodies count too (every authenticated attempt does), and are cheap.
    let attempt = |token: String| {
        post_session_raw(Some(&token), Some("S"), Some("1.6"), b"{not json".to_vec())
    };
    for i in 0..asd_backend::routes::sessions::SESSION_POSTS_PER_MINUTE {
        let (status, _, body) = send(router.clone(), attempt(f.dev())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "attempt {i}: {body}");
    }
    let (status, headers, body) = send(router.clone(), attempt(f.dev())).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(common::content_type(&headers), "application/problem+json");
    assert_eq!(problem_slug(&body), "rate-limited");
    let retry: u64 = headers
        .get(http::header::RETRY_AFTER)
        .expect("Retry-After")
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&retry), "{retry}");
    // A valid post is refused too while over the limit.
    let (status, _, _) = send(router.clone(), post_session(Some(&f.dev()), &golden())).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(f.count("SELECT count(*) FROM sessions").await, 0);

    // The limit is per device: another device of the same org is unaffected.
    let (_, other) = seed_device(&f.db.pool, f.org).await;
    ingest_ok(&router, &other, &golden()).await;
    f.db.cleanup().await;
}

#[tokio::test]
async fn ingest_for_a_withdrawn_participant_is_refused_and_stores_nothing() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    ingest_ok(&router, &f.dev(), &golden_with_id("S-BEFORE")).await;
    sqlx::query("UPDATE participants SET withdrawn_at = now() WHERE token = 'child-SESS-GOL'")
        .execute(&f.db.pool)
        .await
        .unwrap();
    let before = (
        f.count("SELECT count(*) FROM sessions").await,
        f.count("SELECT count(*) FROM idempotency_keys").await,
        f.count("SELECT count(*) FROM audit_log WHERE action = 'session.ingest'")
            .await,
    );

    // A new session, and a replay of the accepted one: 409 problem, no fresh upload URLs.
    for body in [golden_with_id("S-AFTER"), golden_with_id("S-BEFORE")] {
        let (status, headers, resp) =
            send(router.clone(), post_session(Some(&f.dev()), &body)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{resp}");
        assert_eq!(common::content_type(&headers), "application/problem+json");
        assert_eq!(problem_slug(&resp), "participant-withdrawn");
        assert!(
            resp["detail"].as_str().unwrap().contains("withdrawn"),
            "{resp}"
        );
        assert!(resp.get("frames_upload_url").is_none());
    }
    let after = (
        f.count("SELECT count(*) FROM sessions").await,
        f.count("SELECT count(*) FROM idempotency_keys").await,
        f.count("SELECT count(*) FROM audit_log WHERE action = 'session.ingest'")
            .await,
    );
    assert_eq!(after, before);

    // Blob uploads for the participant's earlier session are refused the same way.
    let put = http::Request::builder()
        .method("PUT")
        .uri("/v1/sessions/S-BEFORE/blobs/frames")
        .header("content-type", "application/octet-stream")
        .header(http::header::AUTHORIZATION, bearer(&f.dev()))
        .body(axum::body::Body::from(vec![1u8]))
        .unwrap();
    let (status, _, resp) = send(router.clone(), put).await;
    assert_eq!(status, StatusCode::CONFLICT, "{resp}");
    assert_eq!(problem_slug(&resp), "participant-withdrawn");

    // Other participants of the org are unaffected.
    let mut other = golden_with_id("S-OTHER");
    other["participantToken"] = json!("child-other");
    other["rawPayload"]["participantId"] = json!("child-other");
    ingest_ok(&router, &f.dev(), &other).await;
    f.db.cleanup().await;
}

// ---- schema 1.7 (WP-G.11: SCH-2, SCH-4, QS-5) ---------------------------------------------

/// The tracking columns of one stored session, in SCH-4 order.
async fn tracking_columns(
    f: &Fixture,
    id: &str,
) -> (
    Option<String>,
    Option<f64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let row = sqlx::query(
        "SELECT tracking_verdict, tracking_accuracy_deg, calibration_quality, \
                calibration_model, tracking_pipeline_version, device_model \
         FROM sessions WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    (
        row.get(0),
        row.get(1),
        row.get(2),
        row.get(3),
        row.get(4),
        row.get(5),
    )
}

async fn stored_distance(f: &Fixture, id: &str) -> Option<f64> {
    sqlx::query_scalar("SELECT calibrated_distance_cm FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(&f.db.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn v1_6_post_is_still_201_with_null_tracking_columns() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_ok(&f.router(), &f.dev(), &golden()).await;
    assert_eq!(
        tracking_columns(&f, "SESS-GOLDEN-0001").await,
        (None, None, None, None, None, None)
    );
    // 1.6 keeps the distance it sent (unchanged behaviour).
    assert_eq!(stored_distance(&f, "SESS-GOLDEN-0001").await, Some(38.0));
    f.db.cleanup().await;
}

#[tokio::test]
async fn v1_6_payload_with_an_unknown_tracking_key_is_accepted_as_before() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    // 1.6 allows unknown rawPayload keys; a 1.6 body is never read as 1.7 (SCH-2).
    let mut body = golden();
    body["rawPayload"]["trackingQuality"] = json!({"verdict": "excellent"});
    body["perVideo"][0]["details"]["meanSampleConfidence"] = json!("n/a");
    let (status, resp) = post_as(&f, &body).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    assert_eq!(
        tracking_columns(&f, "SESS-GOLDEN-0001").await,
        (None, None, None, None, None, None)
    );
    let summary: Value =
        sqlx::query_scalar("SELECT summary FROM sessions WHERE id = 'SESS-GOLDEN-0001'")
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert_eq!(summary, body["rawPayload"]);
    f.db.cleanup().await;
}

#[tokio::test]
async fn v1_7_post_is_201_with_derived_tracking_columns() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let payload = golden_v1_7();
    let (status, _, resp) = send(f.router(), post_session_v1_7(Some(&f.dev()), &payload)).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    assert_eq!(resp["status"], "accepted");
    assert_eq!(resp["duplicate"], false);
    assert_eq!(
        tracking_columns(&f, "SESS-GOLDEN-0001").await,
        (
            Some("good".into()),
            Some(1.63),
            Some("good".into()),
            Some("screenAffine".into()),
            Some("asd-gaze/1.0.0".into()),
            Some("iPad13,18".into()),
        )
    );
    let row =
        sqlx::query("SELECT schema_version, summary FROM sessions WHERE id = 'SESS-GOLDEN-0001'")
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert_eq!(row.get::<String, _>("schema_version"), "1.7");
    // The record is stored exactly as received (ING-2).
    assert_eq!(row.get::<Value, _>("summary"), payload["rawPayload"]);
    // Clip rows and details as for 1.6; details keep the QS-3 fields.
    assert_eq!(f.count("SELECT count(*) FROM session_clip_scores").await, 2);
    let details: Value = sqlx::query_scalar(
        "SELECT details FROM session_clip_scores WHERE session_id = 'SESS-GOLDEN-0001' \
         AND clip_order = 1",
    )
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    assert_eq!(details, payload["perVideo"][0]["details"]);
    f.db.cleanup().await;
}

#[tokio::test]
async fn v1_7_calibrated_distance_is_the_calibration_pose_never_a_default() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    // QS-5: the measured pose distance, whatever the top-level value says.
    let mut measured = golden_v1_7_with_id("S-POSE");
    measured["calibratedDistanceCm"] = json!(35);
    measured["rawPayload"]["calibratedDistanceCm"] = json!(35);
    measured["rawPayload"]["trackingQuality"]["calibration"]["pose"]["distanceCm"] = json!(41.5);
    ingest_v1_7_ok(&f.router(), &f.dev(), &measured).await;
    assert_eq!(stored_distance(&f, "S-POSE").await, Some(41.5));

    // Skipped calibration: no distance, even though the app sent its 35 cm default.
    let mut skipped = golden_v1_7_with_id("S-SKIP");
    skipped["calibratedDistanceCm"] = json!(35);
    skipped["rawPayload"]["calibratedDistanceCm"] = json!(35);
    skipped["rawPayload"]["trackingQuality"]["calibration"]["quality"] = json!("skipped");
    ingest_v1_7_ok(&f.router(), &f.dev(), &skipped).await;
    assert_eq!(stored_distance(&f, "S-SKIP").await, None);

    // No tracking-quality record: nothing measured, nothing stored.
    let mut no_record = golden_v1_7_with_id("S-NOREC");
    no_record["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("trackingQuality");
    ingest_v1_7_ok(&f.router(), &f.dev(), &no_record).await;
    assert_eq!(stored_distance(&f, "S-NOREC").await, None);
    assert_eq!(
        tracking_columns(&f, "S-NOREC").await,
        (None, None, None, None, None, None)
    );
    f.db.cleanup().await;
}

#[tokio::test]
async fn schema_version_must_be_1_6_or_1_7_and_equal_in_header_and_body() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let cases = [
        // (header, body)
        (Some("1.7"), golden()),
        (Some("1.6"), golden_v1_7()),
        (Some("1.8"), golden_v1_7()),
        (None, golden_v1_7()),
    ];
    for (header, body) in cases {
        let (status, _, resp) = send(
            f.router(),
            post_session_raw(
                Some(&f.dev()),
                Some("SESS-GOLDEN-0001"),
                header,
                serde_json::to_vec(&body).unwrap(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{header:?}: {resp}");
        assert_eq!(
            problem_slug(&resp),
            "unsupported-schema-version",
            "{header:?}"
        );
    }
    assert_eq!(f.count("SELECT count(*) FROM sessions").await, 0);
    f.db.cleanup().await;
}

#[tokio::test]
async fn each_version_is_checked_against_its_own_schema() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    // 1.7: a typed trackingQuality with a value outside the contract is rejected.
    let mut bad = golden_v1_7();
    bad["rawPayload"]["trackingQuality"]["verdict"] = json!("excellent");
    let (status, _, resp) = send(f.router(), post_session_v1_7(Some(&f.dev()), &bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert_eq!(problem_slug(&resp), "validation-failed");
    let names = param_names(&resp);
    assert!(
        names
            .iter()
            .any(|n| "/rawPayload/trackingQuality/verdict".starts_with(n.as_str()) && n.len() > 1),
        "{names:?}"
    );

    // 1.6 still requires rawPayload.calibratedDistanceCm; 1.7 does not.
    let mut v16 = golden();
    v16["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("calibratedDistanceCm");
    let (status, resp) = post_as(&f, &v16).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert_eq!(problem_slug(&resp), "validation-failed");

    let mut v17 = golden_v1_7();
    v17["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("calibratedDistanceCm");
    ingest_v1_7_ok(&f.router(), &f.dev(), &v17).await;
    f.db.cleanup().await;
}

#[tokio::test]
async fn identical_replay_of_a_1_7_body_is_200_duplicate_and_changes_nothing() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let router = f.router();
    let first = ingest_v1_7_ok(&router, &f.dev(), &golden_v1_7()).await;
    let before = tracking_columns(&f, "SESS-GOLDEN-0001").await;

    let pretty = serde_json::to_vec_pretty(&golden_v1_7()).unwrap();
    let (status, _, replay) = send(
        router.clone(),
        post_session_raw(
            Some(&f.dev()),
            Some("SESS-GOLDEN-0001"),
            Some("1.7"),
            pretty,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    let mut expected = first.clone();
    expected["duplicate"] = json!(true);
    assert_eq!(replay, expected);
    assert_eq!(f.count("SELECT count(*) FROM sessions").await, 1);
    assert_eq!(f.count("SELECT count(*) FROM review_items").await, 0);
    assert_eq!(tracking_columns(&f, "SESS-GOLDEN-0001").await, before);

    // The same key with the body changed (here: the verdict) is still a 409 (CON-17).
    let changed = golden_v1_7_tracking("SESS-GOLDEN-0001", "poor", Some(4.2));
    let (status, _, resp) = send(router.clone(), post_session_v1_7(Some(&f.dev()), &changed)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{resp}");
    assert_eq!(problem_slug(&resp), "idempotency-key-reused");
    assert_eq!(tracking_columns(&f, "SESS-GOLDEN-0001").await, before);
    f.db.cleanup().await;
}

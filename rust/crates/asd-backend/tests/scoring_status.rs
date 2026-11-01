//! owner: WP-G.12
//!
//! Session schema 1.8 (SCH-5): `POST /v1/sessions` accepts 1.6, 1.7 and 1.8. A 1.8
//! `unscorable` session is stored without score numbers (`social_preference` and `affect`
//! NULL, flag `unreliable`), gets an `unscorable` review item, is read back as
//! `scoring_status: unscorable` with its reason, renders "Not scored" in both reports and
//! never feeds a norm. A 1.8 body breaking the scoring rule is a 400 naming the field. 1.6 and
//! 1.7 sessions keep their behaviour (`scoring_status` NULL).
//!
//! Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod ingest_support;
mod tracking_support;

use std::sync::Arc;

use asd_backend::blob::MemoryBlobStore;
use asd_backend::jobs::runner;
use asd_core::synth::{generate_session, Profile};
use axum::body::Body;
use axum::Router;
use common::{send, TestDb};
use http::StatusCode;
use ingest_support::*;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use tracking_support::*;
use uuid::Uuid;

const SCORE_FIELDS: [&str; 5] = [
    "socialPreferenceScore",
    "socialSmileIntensity",
    "smileReactivityIndex",
    "flatAffectScore",
    "surpriseScoreMean",
];

fn fixture(name: &str) -> Value {
    let path = repo_root().join(format!("rust/crates/asd-contract/tests/fixtures/{name}"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn scored() -> Value {
    fixture("session-v1.8-scored.json")
}

fn unscorable() -> Value {
    fixture("session-v1.8-unscorable.json")
}

/// A 1.6 or 1.7 payload as a 1.8 one: `scored` with its numbers, or `unscorable` without.
fn as_v1_8(mut p: Value, unscorable_reason: Option<Option<&str>>) -> Value {
    p["schemaVersion"] = json!("1.8");
    p["rawPayload"]["schemaVersion"] = json!("v1.8");
    let status = if unscorable_reason.is_some() {
        "unscorable"
    } else {
        "scored"
    };
    p["scoringStatus"] = json!(status);
    p["rawPayload"]["scoringStatus"] = json!(status);
    if let Some(reason) = unscorable_reason {
        p.as_object_mut().unwrap().remove("socialPreferenceScore");
        let raw = p["rawPayload"].as_object_mut().unwrap();
        for key in SCORE_FIELDS {
            raw.remove(key);
        }
        if let Some(r) = reason {
            p["unscorableReason"] = json!(r);
            p["rawPayload"]["unscorableReason"] = json!(r);
        }
        p["attentionFlag"] = Value::Null;
    }
    p
}

fn post_v1_8(device: &str, body: &Value) -> http::Request<Body> {
    post_session_raw(
        Some(device),
        body.get("sessionId").and_then(Value::as_str),
        Some("1.8"),
        serde_json::to_vec(body).unwrap(),
    )
}

struct Fixture {
    db: TestDb,
    org: Uuid,
    device: String,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let db = TestDb::migrated().await?;
        let org = seed_quiet_org(&db.pool).await;
        let (_, device) = seed_device(&db.pool, org).await;
        Some(Self { db, org, device })
    }

    fn router(&self) -> Router {
        router(&self.db.pool, false)
    }

    async fn post(&self, body: &Value) -> (StatusCode, Value) {
        let (s, _, b) = send(self.router(), post_v1_8(&self.device, body)).await;
        (s, b)
    }

    async fn user(&self, role: &str) -> String {
        seed_user(&self.db.pool, Some(self.org), role).await.1
    }
}

fn router(pool: &PgPool, allow_synthetic: bool) -> Router {
    app(state_with(
        pool.clone(),
        Arc::new(MemoryBlobStore::new()),
        config(allow_synthetic),
    ))
}

async fn stored(pool: &PgPool, id: &str) -> sqlx::postgres::PgRow {
    sqlx::query(
        "SELECT schema_version, scoring_status, unscorable_reason, social_preference, affect,
                attention_flag, summary
         FROM sessions WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn reasons(pool: &PgPool, id: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT reason FROM review_items WHERE session_id = $1 ORDER BY reason")
        .bind(id)
        .fetch_all(pool)
        .await
        .unwrap()
}

fn invalid_names(body: &Value) -> Vec<String> {
    body["invalid_params"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| p["name"].as_str().unwrap_or("").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

// ---- ingest ---------------------------------------------------------------------------------

#[tokio::test]
async fn scored_1_8_session_is_stored_like_1_7_plus_its_status() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let body = scored();
    let (status, resp) = f.post(&body).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    let row = stored(&f.db.pool, "SESS-GOLDEN-0018").await;
    assert_eq!(row.get::<String, _>("schema_version"), "1.8");
    assert_eq!(
        row.get::<Option<String>, _>("scoring_status").as_deref(),
        Some("scored")
    );
    assert_eq!(row.get::<Option<String>, _>("unscorable_reason"), None);
    assert_eq!(row.get::<Option<f64>, _>("social_preference"), Some(0.62));
    assert_eq!(
        row.get::<Option<Value>, _>("affect").unwrap()["flatAffectScore"],
        json!(0.12)
    );
    assert_eq!(
        row.get::<String, _>("attention_flag"),
        "yellow",
        "device flag kept"
    );
    assert_eq!(
        row.get::<Value, _>("summary"),
        body["rawPayload"],
        "stored as received"
    );
    assert!(!reasons(&f.db.pool, "SESS-GOLDEN-0018")
        .await
        .contains(&"unscorable".to_owned()));
    f.db.cleanup().await;
}

#[tokio::test]
async fn unscorable_session_has_no_numbers_an_unreliable_flag_and_a_review_item() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let body = unscorable();
    let (status, resp) = f.post(&body).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    let id = "SESS-GOLDEN-0019";
    let row = stored(&f.db.pool, id).await;
    assert_eq!(
        row.get::<Option<String>, _>("scoring_status").as_deref(),
        Some("unscorable")
    );
    assert_eq!(
        row.get::<Option<String>, _>("unscorable_reason").as_deref(),
        Some("aggregation_failed")
    );
    assert_eq!(row.get::<Option<f64>, _>("social_preference"), None);
    assert_eq!(row.get::<Option<Value>, _>("affect"), None);
    assert_eq!(row.get::<String, _>("attention_flag"), "unreliable");
    assert_eq!(reasons(&f.db.pool, id).await, ["unscorable"]);

    // Read back: detail, list and review queue.
    let user = f.user("clinician").await;
    let (status, _, d) = send(f.router(), user_get(&user, &format!("/v1/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::OK, "{d}");
    assert_eq!(d["scoring_status"], "unscorable");
    assert_eq!(d["unscorable_reason"], "aggregation_failed");
    assert_eq!(d["social_preference"], Value::Null);
    assert_eq!(d["affect"], Value::Null);
    assert_eq!(d["schema_version"], "1.8");
    assert_eq!(d["attention_flag"], "unreliable");
    let (status, _, page) = send(
        f.router(),
        user_get(&user, &format!("/v1/orgs/{}/sessions", f.org)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["items"][0]["scoring_status"], "unscorable");
    assert_eq!(page["items"][0]["unscorable_reason"], "aggregation_failed");
    let reviewer = f.user("reviewer").await;
    let (status, _, review) = send(
        f.router(),
        user_get(&reviewer, &format!("/v1/orgs/{}/review", f.org)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{review}");
    assert!(review["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["reason"] == "unscorable" && i["session_id"] == id));
    f.db.cleanup().await;
}

#[tokio::test]
async fn unscorable_without_a_reason_and_with_null_scores_is_accepted() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = as_v1_8(golden_v1_7_with_id("S-NULLS"), Some(None));
    body["socialPreferenceScore"] = Value::Null;
    for key in SCORE_FIELDS {
        body["rawPayload"][key] = Value::Null;
    }
    let (status, resp) = f.post(&body).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    let row = stored(&f.db.pool, "S-NULLS").await;
    assert_eq!(
        row.get::<Option<String>, _>("scoring_status").as_deref(),
        Some("unscorable")
    );
    assert_eq!(row.get::<Option<String>, _>("unscorable_reason"), None);
    f.db.cleanup().await;
}

#[tokio::test]
async fn scoring_rule_violations_are_400_naming_the_field() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    // Scored without a score (schema rule).
    let mut missing = scored();
    missing["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("flatAffectScore");
    // Unscorable with a placeholder number (schema rule).
    let mut placeholder = unscorable();
    placeholder["socialPreferenceScore"] = json!(0);
    // Unscorable with a device flag (typed rule only).
    let mut flagged = unscorable();
    flagged["attentionFlag"] = json!("red");
    // Status disagreement between the request and the record (typed rule).
    let mut mismatch = unscorable();
    mismatch["rawPayload"]["unscorableReason"] = json!("other");
    // No status at all.
    let mut no_status = scored();
    no_status.as_object_mut().unwrap().remove("scoringStatus");
    for (name, body, field) in [
        ("missing", missing, "/rawPayload"),
        ("placeholder", placeholder, "/socialPreferenceScore"),
        ("flagged", flagged, "/attentionFlag"),
        ("mismatch", mismatch, "/rawPayload/unscorableReason"),
        ("no_status", no_status, "/scoringStatus"),
    ] {
        let (status, resp) = f.post(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name}: {resp}");
        assert_eq!(problem_slug(&resp), "validation-failed", "{name}");
        assert!(
            invalid_names(&resp).iter().any(|n| n.starts_with(field)),
            "{name}: {resp}"
        );
    }
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "nothing stored");
    f.db.cleanup().await;
}

#[tokio::test]
async fn versions_must_match_and_1_7_is_unchanged() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    // A 1.8 body under a 1.7 header, and a 1.7 body under a 1.8 header.
    let (status, _, resp) = send(f.router(), post_session_v1_7(Some(&f.device), &scored())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&resp), "unsupported-schema-version");
    let (status, resp) = f.post(&golden_v1_7()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&resp), "unsupported-schema-version");

    // 1.7 and 1.6 still go in, with no scoring status and no unscorable item.
    ingest_v1_7_ok(&f.router(), &f.device, &golden_v1_7()).await;
    ingest_ok(&f.router(), &f.device, &golden_with_id("SESS-16")).await;
    for (id, version) in [("SESS-GOLDEN-0001", "1.7"), ("SESS-16", "1.6")] {
        let row = stored(&f.db.pool, id).await;
        assert_eq!(row.get::<String, _>("schema_version"), version);
        assert_eq!(row.get::<Option<String>, _>("scoring_status"), None, "{id}");
        assert_eq!(row.get::<Option<f64>, _>("social_preference"), Some(0.62));
        assert!(row.get::<Option<Value>, _>("affect").is_some());
        assert!(!reasons(&f.db.pool, id)
            .await
            .contains(&"unscorable".to_owned()));
    }
    let user = f.user("viewer").await;
    let (_, _, d) = send(f.router(), user_get(&user, "/v1/sessions/SESS-GOLDEN-0001")).await;
    assert_eq!(d["scoring_status"], Value::Null);
    assert_eq!(d["social_preference"], json!(0.62));
    f.db.cleanup().await;
}

#[tokio::test]
async fn the_database_refuses_an_unscored_row_without_the_status() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_v1_7_ok(&f.router(), &f.device, &golden_v1_7()).await;
    let err =
        sqlx::query("UPDATE sessions SET social_preference = NULL WHERE id = 'SESS-GOLDEN-0001'")
            .execute(&f.db.pool)
            .await;
    assert!(err.is_err(), "a scored (or pre-1.8) row keeps its score");
    let err = sqlx::query(
        "UPDATE sessions SET unscorable_reason = 'other' WHERE id = 'SESS-GOLDEN-0001'",
    )
    .execute(&f.db.pool)
    .await;
    assert!(err.is_err(), "a reason needs the unscorable status");
    f.db.cleanup().await;
}

// ---- reports --------------------------------------------------------------------------------

#[tokio::test]
async fn reports_say_not_scored() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let (status, resp) = f.post(&unscorable()).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    let user = f.user("clinician").await;
    let (status, _, bytes) = send_raw(
        f.router(),
        user_get(&user, "/v1/sessions/SESS-GOLDEN-0019/report.pdf"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let record = asd_export::stored_record(unscorable()["rawPayload"].clone()).unwrap();
    let expected = asd_export::pdf::render(&asd_export::pdf::DoctorReportInput {
        participant_label: record.participant_id.clone(),
        child_age_months: None,
        guardian_name: asd_export::pdf::DEFAULT_GUARDIAN.into(),
        assessment_date: record.started_at.clone(),
        gaze_record: Some(&record),
        mchat: None,
        care: None,
    })
    .unwrap();
    assert_eq!(bytes, expected);
    assert!(String::from_utf8_lossy(&bytes).contains("NOT SCORED (AGGREGATION FAILED)"));

    let (status, _, bytes) = send_raw(
        f.router(),
        user_get(&user, "/v1/sessions/SESS-GOLDEN-0019/fhir"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let bundle: Value = serde_json::from_slice(&bytes).unwrap();
    let text = bundle.to_string();
    assert!(!text.contains("social-preference-score"));
    assert!(!text.contains("flat-affect-score"));
    assert!(text.contains("Not scored (aggregation failed)"));
    f.db.cleanup().await;
}

// ---- norms ----------------------------------------------------------------------------------

/// A consented device capture (synthetic generator, made `truedepth`).
fn device_session(index: u64) -> Value {
    let mut p = (0..)
        .map(|i| generate_session(7, index * 50 + i, Some(Profile::Typical)))
        .find(|p| p["qualityVerdict"] == "optimal")
        .unwrap();
    p["captureSource"] = json!("truedepth");
    p["rawPayload"]["captureSource"] = json!("truedepth");
    let raw = p["rawPayload"].as_object_mut().unwrap();
    for key in [
        "synthetic",
        "generator",
        "generatorVersion",
        "seed",
        "index",
        "profile",
    ] {
        raw.remove(key);
    }
    raw.insert("normCollection".into(), json!({"consented": true}));
    p
}

#[tokio::test]
async fn unscorable_sessions_never_feed_a_norm() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let pool = &f.db.pool;
    let admin = f.user("admin").await;
    let reviewer = f.user("reviewer").await;
    let scored = as_v1_8(device_session(1), None);
    let unscored = as_v1_8(device_session(2), Some(Some("insufficient_valid_gaze")));
    for body in [&scored, &unscored] {
        let (status, resp) = f.post(body).await;
        assert_eq!(status, StatusCode::CREATED, "{resp}");
    }
    // Close every review item of both, so only the scoring status can exclude one.
    let items: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM review_items")
        .fetch_all(pool)
        .await
        .unwrap();
    assert!(
        items.len() >= 3,
        "two norm_candidate items and one unscorable item"
    );
    for id in items {
        let (status, _, resp) = send(
            f.router(),
            user_json(
                &reviewer,
                "PATCH",
                &format!("/v1/review/{id}"),
                &json!({"status": "done"}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{resp}");
    }

    let (status, _, job) = send(
        f.router(),
        user_json(&admin, "POST", "/v1/jobs/norm-aggregate", &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    assert_eq!(runner::run_pending(pool, "test-worker").await.unwrap(), 1);
    let (_, _, job) = send(
        f.router(),
        user_get(&admin, &format!("/v1/jobs/{}", job["id"].as_str().unwrap())),
    )
    .await;
    assert_eq!(job["status"], "succeeded", "{job}");
    let run_id = job["result"]["norm_run_id"].as_str().unwrap();
    let (_, _, run) = send(
        f.router(),
        user_get(&admin, &format!("/v1/norm-runs/{run_id}")),
    )
    .await;
    assert_eq!(run["n_total"], 1, "only the scored session: {run}");
    f.db.cleanup().await;
}

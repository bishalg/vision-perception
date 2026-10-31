//! owner: WP-B.3
//!
//! ING-3 review auto-rules (written in the ingest transaction) and the review queue API:
//! `GET/POST /v1/orgs/{id}/review`, `PATCH /v1/review/{id}`.

mod common;
mod ingest_support;
mod tracking_support;

use asd_backend::ingest::auto_review::audit_draw;
use asd_contract::api_v1::{ReviewItem, ReviewPage, ReviewReason, ReviewStatus};
use common::{send, TestDb};
use http::StatusCode;
use ingest_support::*;
use serde_json::{json, Value};
use sqlx::Row;
use tracking_support::*;
use uuid::Uuid;

struct Fixture {
    db: TestDb,
    org: Uuid,
    token: String,
    router: axum::Router,
}

impl Fixture {
    async fn with_settings(settings: Value) -> Option<Self> {
        let db = TestDb::migrated().await?;
        let org = seed_org(&db.pool, settings).await;
        let (_, token) = seed_device(&db.pool, org).await;
        let router = app(state(db.pool.clone()));
        Some(Self {
            db,
            org,
            token,
            router,
        })
    }
    async fn new() -> Option<Self> {
        Self::with_settings(json!({"audit_rate": 0.0})).await
    }
    /// The device's bearer token.
    fn dev(&self) -> String {
        self.token.clone()
    }
    async fn reasons(&self, session: &str) -> Vec<String> {
        sqlx::query_scalar("SELECT reason FROM review_items WHERE session_id = $1 ORDER BY reason")
            .bind(session)
            .fetch_all(&self.db.pool)
            .await
            .unwrap()
    }
    /// A logged-in user of this org with `role`; returns the access token.
    async fn user(&self, role: &str) -> String {
        seed_user(&self.db.pool, Some(self.org), role).await.1
    }
}

#[test]
fn audit_draw_is_deterministic_and_uniformish() {
    assert_eq!(
        audit_draw("SESS-GOLDEN-0001"),
        audit_draw("SESS-GOLDEN-0001")
    );
    assert_ne!(audit_draw("S-1"), audit_draw("S-2"));
    let n = 2000;
    let hits = (0..n)
        .filter(|i| audit_draw(&format!("S-{i}")) < 0.05)
        .count();
    // 5 % of 2000 = 100; a fair hash lands well inside 50..150.
    assert!((50..150).contains(&hits), "{hits}");
    assert!((0..n).all(|i| (0.0..1.0).contains(&audit_draw(&format!("S-{i}")))));
}

#[tokio::test]
async fn golden_session_creates_no_review_item() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    assert!(f.reasons("SESS-GOLDEN-0001").await.is_empty());
    f.db.cleanup().await;
}

#[tokio::test]
async fn low_quality_red_flag_and_norm_candidate_rules() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut low = golden_with_id("S-LOW");
    low["validRatio"] = json!(0.79);
    ingest_ok(&f.router, &f.dev(), &low).await;
    assert_eq!(f.reasons("S-LOW").await, ["low_quality"]);

    let mut edge = golden_with_id("S-EDGE");
    edge["validRatio"] = json!(0.80);
    ingest_ok(&f.router, &f.dev(), &edge).await;
    assert!(f.reasons("S-EDGE").await.is_empty());

    let mut red = golden_with_id("S-RED");
    red["attentionFlag"] = json!("red");
    ingest_ok(&f.router, &f.dev(), &red).await;
    assert_eq!(f.reasons("S-RED").await, ["flag_red"]);

    let mut norm = golden_with_id("S-NORM");
    norm["rawPayload"]["normCollection"] = json!({"consented": true});
    ingest_ok(&f.router, &f.dev(), &norm).await;
    assert_eq!(f.reasons("S-NORM").await, ["norm_candidate"]);

    let mut not_consented = golden_with_id("S-NC");
    not_consented["rawPayload"]["normCollection"] = json!({"consented": "yes"});
    ingest_ok(&f.router, &f.dev(), &not_consented).await;
    assert!(f.reasons("S-NC").await.is_empty());

    // Unreliable (server-derived) is not red; low validRatio is low_quality.
    let mut unreliable = golden_with_id("S-UNR");
    unreliable["attentionFlag"] = Value::Null;
    unreliable["validRatio"] = json!(0.5);
    ingest_ok(&f.router, &f.dev(), &unreliable).await;
    assert_eq!(f.reasons("S-UNR").await, ["low_quality"]);

    let org: Uuid = sqlx::query_scalar("SELECT DISTINCT org_id FROM review_items")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(org, f.org);
    let open: i64 = sqlx::query_scalar("SELECT count(*) FROM review_items WHERE status <> 'open'")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(open, 0);
    f.db.cleanup().await;
}

#[tokio::test]
async fn random_audit_follows_the_org_rate() {
    let Some(f) = Fixture::with_settings(json!({"audit_rate": 1.0})).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    assert_eq!(f.reasons("SESS-GOLDEN-0001").await, ["random_audit"]);
    f.db.cleanup().await;

    // Default rate 0.05: the draw decides, deterministically.
    let Some(f) = Fixture::with_settings(json!({})).await else {
        return;
    };
    let mut audited = 0;
    for i in 0..40 {
        let id = format!("S-{i}");
        ingest_ok(&f.router, &f.dev(), &golden_with_id(&id)).await;
        let expected = audit_draw(&id) < 0.05;
        assert_eq!(
            f.reasons(&id).await.contains(&"random_audit".to_owned()),
            expected,
            "{id}"
        );
        audited += usize::from(expected);
    }
    assert!(audited < 40);
    f.db.cleanup().await;
}

#[tokio::test]
async fn review_queue_lists_org_items_with_filters() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut red = golden_with_id("S-RED");
    red["attentionFlag"] = json!("red");
    red["validRatio"] = json!(0.7);
    ingest_ok(&f.router, &f.dev(), &red).await;
    let viewer = f.user("viewer").await;
    let (status, _, body) = send(
        f.router.clone(),
        user_get(&viewer, &format!("/v1/orgs/{}/review", f.org)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: ReviewPage = serde_json::from_value(body).unwrap();
    assert_eq!(page.total, Some(2));
    let mut reasons: Vec<_> = page.items.iter().map(|i| i.reason).collect();
    reasons.sort_by_key(|r| format!("{r:?}"));
    assert_eq!(reasons, [ReviewReason::FlagRed, ReviewReason::LowQuality]);
    assert!(page
        .items
        .iter()
        .all(|i| i.status == ReviewStatus::Open && i.session_id == "S-RED"));
    // The list carries a short session summary, so the console needs no call per item.
    for i in &page.items {
        let s = i.session.as_ref().expect("session summary");
        assert_eq!(
            s.attention_flag,
            asd_contract::api_v1::SessionAttentionFlag::Red
        );
        assert_eq!(s.participant_token, "child-SESS-GOL");
        assert!(s.started_at.ends_with('Z'), "{}", s.started_at);
        assert!(!s.quality_verdict.is_empty());
    }

    let (_, _, body) = send(
        f.router.clone(),
        user_get(
            &viewer,
            &format!("/v1/orgs/{}/review?reason=flag_red", f.org),
        ),
    )
    .await;
    assert_eq!(body["total"], 1);
    let (_, _, body) = send(
        f.router.clone(),
        user_get(&viewer, &format!("/v1/orgs/{}/review?status=done", f.org)),
    )
    .await;
    assert_eq!(body["total"], 0);

    let other_org = seed_quiet_org(&f.db.pool).await;
    let (_, outsider) = seed_user(&f.db.pool, Some(other_org), "owner").await;
    let (status, _, _) = send(
        f.router.clone(),
        user_get(&outsider, &format!("/v1/orgs/{}/review", f.org)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    f.db.cleanup().await;
}

#[tokio::test]
async fn manual_enqueue_creates_a_manual_item_once() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let clinician = f.user("clinician").await;
    let uri = format!("/v1/orgs/{}/review", f.org);
    let body = json!({"session_id": "SESS-GOLDEN-0001", "note": "second look"});
    let (status, _, created) =
        send(f.router.clone(), user_json(&clinician, "POST", &uri, &body)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let item: ReviewItem = serde_json::from_value(created.clone()).unwrap();
    assert_eq!(item.reason, ReviewReason::Manual);
    assert_eq!(item.status, ReviewStatus::Open);
    assert_eq!(item.org_id, f.org.to_string());

    // Enqueuing again returns the existing item.
    let (status, _, again) =
        send(f.router.clone(), user_json(&clinician, "POST", &uri, &body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["id"], created["id"]);

    // Unknown session, or another org's: 404.
    let (status, _, _) = send(
        f.router.clone(),
        user_json(&clinician, "POST", &uri, &json!({"session_id": "NOPE"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Viewers cannot enqueue.
    let viewer = f.user("viewer").await;
    let (status, _, b) = send(f.router.clone(), user_json(&viewer, "POST", &uri, &body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{b}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn patch_review_records_decision_labels_and_audit() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut red = golden_with_id("S-RED");
    red["attentionFlag"] = json!("red");
    ingest_ok(&f.router, &f.dev(), &red).await;
    let item_id: Uuid = sqlx::query_scalar("SELECT id FROM review_items")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    let (reviewer_id, reviewer) = seed_user(&f.db.pool, Some(f.org), "reviewer").await;
    let uri = format!("/v1/review/{item_id}");

    let patch = json!({
        "status": "done",
        "assigned_to": reviewer_id.to_string(),
        "decision": {"outcome": "confirmed"},
        "labels": [{"key": "gaze_quality", "value": "good", "clip_id": "stim_05_geopref_split_10s"}]
    });
    let (status, _, body) = send(
        f.router.clone(),
        user_json(&reviewer, "PATCH", &uri, &patch),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item: ReviewItem = serde_json::from_value(body).unwrap();
    assert_eq!(item.status, ReviewStatus::Done);
    assert_eq!(item.decision, Some(json!({"outcome": "confirmed"})));
    assert_eq!(
        item.decided_by.as_deref(),
        Some(reviewer_id.to_string().as_str())
    );
    assert!(item.decided_at.is_some());
    assert_eq!(
        item.assigned_to.as_deref(),
        Some(reviewer_id.to_string().as_str())
    );

    let label = sqlx::query("SELECT * FROM labels")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(label.get::<String, _>("session_id"), "S-RED");
    assert_eq!(label.get::<String, _>("key"), "gaze_quality");
    assert_eq!(label.get::<Value, _>("value"), json!("good"));
    assert_eq!(
        label.get::<Option<Uuid>, _>("created_by"),
        Some(reviewer_id)
    );

    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'review.update' AND actor_type = 'user' AND target_id = $1",
    )
    .bind(item_id.to_string())
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
    f.db.cleanup().await;
}

#[tokio::test]
async fn patch_review_is_role_and_org_checked() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut red = golden_with_id("S-RED");
    red["attentionFlag"] = json!("red");
    ingest_ok(&f.router, &f.dev(), &red).await;
    let item_id: Uuid = sqlx::query_scalar("SELECT id FROM review_items")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    let uri = format!("/v1/review/{item_id}");
    let patch = json!({"status": "in_review"});

    let viewer = f.user("viewer").await;
    let (status, _, _) = send(f.router.clone(), user_json(&viewer, "PATCH", &uri, &patch)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let other_org = seed_quiet_org(&f.db.pool).await;
    let (outsider, outsider_token) = seed_user(&f.db.pool, Some(other_org), "owner").await;
    let (status, _, _) = send(
        f.router.clone(),
        user_json(&outsider_token, "PATCH", &uri, &patch),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // assigned_to must be a user of the same org.
    let reviewer = f.user("reviewer").await;
    let (status, _, body) = send(
        f.router.clone(),
        user_json(
            &reviewer,
            "PATCH",
            &uri,
            &json!({"assigned_to": outsider.to_string()}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(problem_slug(&body), "validation-failed");

    let (status, _, _) = send(
        f.router.clone(),
        user_json(
            &reviewer,
            "PATCH",
            &format!("/v1/review/{}", Uuid::new_v4()),
            &patch,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _, body) = send(
        f.router.clone(),
        user_json(&reviewer, "PATCH", &uri, &patch),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "in_review");
    assert_eq!(body["decided_at"], Value::Null);
    f.db.cleanup().await;
}

// ---- low_tracking_quality (WP-G.11, SCH-4) -------------------------------------------------

#[tokio::test]
async fn poor_or_unusable_tracking_opens_low_tracking_quality_in_the_ingest_transaction() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    for (id, verdict, accuracy, expected) in [
        ("S-GOOD", "good", Some(1.63), false),
        ("S-FAIR", "fair", Some(3.0), false),
        ("S-POOR", "poor", Some(4.2), true),
        ("S-UNUSABLE", "unusable", None, true),
        // Server re-check against ScoringProfile.trackingQuality.verdict.accuracyBands
        // (default fairMaxDeg 3.5): a device-side `fair` above the band is still poor.
        ("S-FAIR-OVER", "fair", Some(3.6), true),
        ("S-FAIR-EDGE", "fair", Some(3.5), false),
    ] {
        let body = golden_v1_7_tracking(id, verdict, accuracy);
        ingest_v1_7_ok(&f.router, &f.dev(), &body).await;
        let want: Vec<&str> = if expected {
            vec!["low_tracking_quality"]
        } else {
            vec![]
        };
        assert_eq!(f.reasons(id).await, want, "{id}");
    }

    // Written with the session, in the same transaction: the ingest audit row lists it.
    let after: Value = sqlx::query_scalar(
        "SELECT after FROM audit_log WHERE action = 'session.ingest' AND target_id = 'S-POOR'",
    )
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    assert_eq!(after["review_reasons"], json!(["low_tracking_quality"]));

    // Beside the other rules, in the fixed order.
    let mut both = golden_v1_7_tracking("S-BOTH", "poor", Some(4.2));
    both["validRatio"] = json!(0.7);
    ingest_v1_7_ok(&f.router, &f.dev(), &both).await;
    let after: Value = sqlx::query_scalar(
        "SELECT after FROM audit_log WHERE action = 'session.ingest' AND target_id = 'S-BOTH'",
    )
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    assert_eq!(
        after["review_reasons"],
        json!(["low_quality", "low_tracking_quality"])
    );

    // A 1.6 session never gets it, whatever its rawPayload carries.
    let mut v16 = golden_with_id("S-V16");
    v16["rawPayload"]["trackingQuality"] = json!({"verdict": "unusable"});
    ingest_ok(&f.router, &f.dev(), &v16).await;
    assert!(f.reasons("S-V16").await.is_empty());

    // The queue lists and filters the new reason; items decode as the contract type.
    let viewer = f.user("viewer").await;
    let (status, _, body) = send(
        f.router.clone(),
        user_get(
            &viewer,
            &format!("/v1/orgs/{}/review?reason=low_tracking_quality", f.org),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: ReviewPage = serde_json::from_value(body).unwrap();
    assert_eq!(page.total, Some(4));
    assert!(page
        .items
        .iter()
        .all(|i| i.reason == ReviewReason::LowTrackingQuality));
    f.db.cleanup().await;
}

#[tokio::test]
async fn low_tracking_quality_uses_the_published_scoring_profile_thresholds() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    // A published profile that tightens the fair band to 3.0 degrees.
    let mut profile =
        serde_json::to_value(asd_contract::config::ScoringProfile::default()).unwrap();
    profile["trackingQuality"]["verdict"]["accuracyBands"]["fairMaxDeg"] = json!(3.0);
    sqlx::query(
        "INSERT INTO config_documents (kind, version, status, body, published_at) \
         VALUES ('scoring_profile', 'g11-test', 'published', $1, now())",
    )
    .bind(&profile)
    .execute(&f.db.pool)
    .await
    .unwrap();
    ingest_v1_7_ok(
        &f.router,
        &f.dev(),
        &golden_v1_7_tracking("S-TIGHT", "fair", Some(3.2)),
    )
    .await;
    assert_eq!(f.reasons("S-TIGHT").await, ["low_tracking_quality"]);
    ingest_v1_7_ok(
        &f.router,
        &f.dev(),
        &golden_v1_7_tracking("S-OK", "fair", Some(2.9)),
    )
    .await;
    assert!(f.reasons("S-OK").await.is_empty());
    f.db.cleanup().await;
}

#[tokio::test]
async fn migration_0013_constrains_tracking_values_and_review_reasons() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_v1_7_ok(&f.router, &f.dev(), &golden_v1_7()).await;
    for sql in [
        "UPDATE sessions SET tracking_verdict = 'excellent'",
        "UPDATE sessions SET calibration_quality = 'great'",
        "UPDATE sessions SET tracking_accuracy_deg = -1",
    ] {
        assert!(sqlx::query(sql).execute(&f.db.pool).await.is_err(), "{sql}");
    }
    sqlx::query(
        "INSERT INTO review_items (org_id, session_id, reason) \
         VALUES ($1, 'SESS-GOLDEN-0001', 'low_tracking_quality')",
    )
    .bind(f.org)
    .execute(&f.db.pool)
    .await
    .expect("low_tracking_quality is an allowed reason");
    assert!(sqlx::query(
        "INSERT INTO review_items (org_id, session_id, reason) \
         VALUES ($1, 'SESS-GOLDEN-0001', 'bogus')",
    )
    .bind(f.org)
    .execute(&f.db.pool)
    .await
    .is_err());
    f.db.cleanup().await;
}

//! owner: WP-B.3
//!
//! Console reads (API-2, API-3, CON-16, CON-19, CON-20): org-scoped session list with
//! filters and cursor pagination, session detail, clips with per-metric `rows[]`, org
//! metrics and participants. Another org's resources answer 404 (AUTH-4). Every body
//! decodes into the `asd-contract::api_v1` type.

mod common;
mod ingest_support;

use asd_contract::api_v1::{
    ClipList, Metrics, ParticipantPage, SessionAttentionFlag, SessionDetail, SessionPage,
};
use common::{send, TestDb};
use http::StatusCode;
use ingest_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn console_reads_without_user_identity_are_401() {
    for uri in [
        format!("/v1/orgs/{}/sessions", Uuid::new_v4()),
        "/v1/sessions/S".to_owned(),
        "/v1/sessions/S/clips".to_owned(),
        "/v1/sessions/S/report.pdf".to_owned(),
        format!("/v1/orgs/{}/metrics", Uuid::new_v4()),
        format!("/v1/orgs/{}/participants", Uuid::new_v4()),
    ] {
        let (status, _, body) = send(app(common::state_without_db()), common::get(&uri)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(problem_slug(&body), "unauthorized", "{uri}");
    }
}

/// One session per call: golden payload with a chosen id, start time and labels.
fn session(
    id: &str,
    started: &str,
    flag: Option<&str>,
    quality: &str,
    bundle: Option<&str>,
) -> Value {
    let mut v = golden_with_id(id);
    v["rawPayload"]["startedAt"] = json!(started);
    v["attentionFlag"] = flag.map_or(Value::Null, |f| json!(f));
    v["qualityVerdict"] = json!(quality);
    v["rawPayload"]["qualityVerdict"] = json!(quality);
    if let Some(b) = bundle {
        v["rawPayload"]["bundleVersion"] = json!(b);
    }
    v
}

struct Fixture {
    db: TestDb,
    org: Uuid,
    device: Uuid,
    token: String,
    user: String,
    router: axum::Router,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let db = TestDb::migrated().await?;
        let org = seed_quiet_org(&db.pool).await;
        let (device, token) = seed_device(&db.pool, org).await;
        let (_, user) = seed_user(&db.pool, Some(org), "clinician").await;
        let router = app(state(db.pool.clone()));
        Some(Self {
            db,
            org,
            device,
            token,
            user,
            router,
        })
    }
    /// The device's bearer token.
    fn dev(&self) -> String {
        self.token.clone()
    }
    async fn seed_five(&self) {
        for body in [
            session(
                "S-1",
                "2026-10-01T10:00:00.000Z",
                Some("green"),
                "optimal",
                Some("b1"),
            ),
            session(
                "S-2",
                "2026-10-02T10:00:00.000Z",
                Some("red"),
                "acceptable",
                Some("b1"),
            ),
            session(
                "S-3",
                "2026-10-03T10:00:00.000Z",
                Some("yellow"),
                "optimal",
                Some("b2"),
            ),
            session(
                "S-4",
                "2026-10-04T10:00:00.000Z",
                Some("red"),
                "degraded",
                None,
            ),
            session(
                "S-5",
                "2026-10-05T10:00:00.000Z",
                Some("green"),
                "optimal",
                Some("b2"),
            ),
        ] {
            ingest_ok(&self.router, &self.dev(), &body).await;
        }
    }
    async fn list(&self, query: &str) -> (StatusCode, Value) {
        let (s, _, b) = send(
            self.router.clone(),
            user_get(
                &self.user,
                &format!("/v1/orgs/{}/sessions{query}", self.org),
            ),
        )
        .await;
        (s, b)
    }
}

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn session_list_is_newest_first_and_decodes_as_the_contract_page() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.seed_five().await;
    let (status, body) = f.list("").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body), ["S-5", "S-4", "S-3", "S-2", "S-1"]);
    assert_eq!(body["total"], 5);
    assert_eq!(body["next_cursor"], Value::Null);
    let page: SessionPage = serde_json::from_value(body.clone()).unwrap();
    let first = &page.items[0];
    // CON-20: participant_id is the participants UUID; the token is carried too.
    assert!(Uuid::parse_str(&first.participant_id).is_ok());
    assert_eq!(first.participant_token, "child-SESS-GOL");
    assert_eq!(first.attention_flag, SessionAttentionFlag::Green);
    assert_eq!(first.battery, None);
    assert_eq!(first.started_at, "2026-10-05T10:00:00.000Z");
    assert_eq!(
        first.device_id.as_deref(),
        Some(f.device.to_string().as_str())
    );
    assert_eq!(first.reviewed, Some(false));
    assert_eq!(first.composite_confidence, Some(0.5));
    f.db.cleanup().await;
}

#[tokio::test]
async fn session_list_filters() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.seed_five().await;
    for (query, expected) in [
        ("?flag=red", vec!["S-4", "S-2"]),
        ("?quality=optimal", vec!["S-5", "S-3", "S-1"]),
        ("?bundle_version=b2", vec!["S-5", "S-3"]),
        ("?from=2026-10-02&to=2026-10-04", vec!["S-4", "S-3", "S-2"]),
        (
            "?from=2026-10-02T10:00:00Z&to=2026-10-03T10:00:00Z",
            vec!["S-3", "S-2"],
        ),
        ("?flag=green&quality=optimal", vec!["S-5", "S-1"]),
    ] {
        let (status, body) = f.list(query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(ids(&body), expected, "{query}");
        assert_eq!(body["total"], expected.len(), "{query}");
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn reviewed_filter_follows_closed_review_items() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.seed_five().await;
    // S-2 and S-4 are red: each has an open flag_red item. Close S-2's.
    sqlx::query("UPDATE review_items SET status = 'done' WHERE session_id = 'S-2'")
        .execute(&f.db.pool)
        .await
        .unwrap();
    let (_, body) = f.list("?reviewed=true").await;
    assert_eq!(ids(&body), ["S-2"]);
    let (_, body) = f.list("?reviewed=false").await;
    assert_eq!(ids(&body), ["S-5", "S-4", "S-3", "S-1"]);
    f.db.cleanup().await;
}

#[tokio::test]
async fn cursor_pagination_walks_every_item_once() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.seed_five().await;
    let mut seen = Vec::new();
    let mut query = "?limit=2".to_owned();
    loop {
        let (status, body) = f.list(&query).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["items"].as_array().unwrap().len() <= 2);
        seen.extend(ids(&body));
        match body["next_cursor"].as_str() {
            Some(c) => query = format!("?limit=2&cursor={c}"),
            None => break,
        }
    }
    assert_eq!(seen, ["S-5", "S-4", "S-3", "S-2", "S-1"]);
    // Filters apply across pages.
    let (_, body) = f.list("?flag=green&limit=1").await;
    assert_eq!(ids(&body), ["S-5"]);
    let c = body["next_cursor"].as_str().unwrap().to_owned();
    let (_, body) = f.list(&format!("?flag=green&limit=1&cursor={c}")).await;
    assert_eq!(ids(&body), ["S-1"]);
    assert_eq!(body["next_cursor"], Value::Null);
    f.db.cleanup().await;
}

#[tokio::test]
async fn bad_list_parameters_are_400() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    for query in [
        "?limit=101",
        "?limit=0",
        "?cursor=not-a-cursor",
        "?flag=purple",
        "?from=last-week",
        "?reviewed=maybe",
    ] {
        let (status, body) = f.list(query).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
        assert_eq!(problem_slug(&body), "validation-failed", "{query}");
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn another_orgs_sessions_are_404_but_superadmin_reads_any_org() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.seed_five().await;
    let other_org = seed_quiet_org(&f.db.pool).await;
    let (_, oh) = seed_user(&f.db.pool, Some(other_org), "owner").await;
    for uri in [
        format!("/v1/orgs/{}/sessions", f.org),
        "/v1/sessions/S-1".to_owned(),
        "/v1/sessions/S-1/clips".to_owned(),
        format!("/v1/orgs/{}/metrics", f.org),
        format!("/v1/orgs/{}/participants", f.org),
    ] {
        let (status, _, body) = send(f.router.clone(), user_get(&oh, &uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
        assert_eq!(problem_slug(&body), "not-found");
    }
    let (_, ah) = seed_user(&f.db.pool, None, "superadmin").await;
    let (status, _, body) = send(
        f.router.clone(),
        user_get(&ah, &format!("/v1/orgs/{}/sessions", f.org)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 5);
    f.db.cleanup().await;
}

#[tokio::test]
async fn session_detail_carries_stored_measurements() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (status, _, body) = send(
        f.router.clone(),
        user_get(&f.user, "/v1/sessions/SESS-GOLDEN-0001"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let d: SessionDetail = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(d.summary.id, "SESS-GOLDEN-0001");
    assert_eq!(d.org_id, f.org.to_string());
    assert_eq!(d.schema_version.as_deref(), Some("1.6"));
    assert_eq!(d.valid_frame_ratio, Some(0.93));
    assert_eq!(d.eye_dwell, Some(0.33));
    assert_eq!(d.mouth_dwell, None);
    assert_eq!(d.first_eye_latency_ms, None);
    assert_eq!(d.calibrated_distance_cm, Some(38.0));
    assert_eq!(d.social_preference, Some(0.62));
    assert_eq!(d.summary.composite_score, Some(0.74));
    assert_eq!(d.summary.attention_flag, SessionAttentionFlag::Yellow);
    assert_eq!(d.affect.unwrap()["socialSmileIntensity"], json!(0.41));
    // Optional members are present as null, not omitted.
    assert!(body.as_object().unwrap().contains_key("mouth_dwell"));
    assert!(body.as_object().unwrap().contains_key("battery"));

    let (status, _, body) = send(f.router.clone(), user_get(&f.user, "/v1/sessions/NOPE")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn clips_carry_per_metric_rows_and_worst_row_flat_columns() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (status, _, body) = send(
        f.router.clone(),
        user_get(&f.user, "/v1/sessions/SESS-GOLDEN-0001/clips"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let list: ClipList = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(list.items.len(), 2);

    let c = &list.items[0];
    assert_eq!(c.clip_id, "stim_05_geopref_split_10s");
    assert_eq!(c.order, 1);
    assert_eq!(c.rows.len(), 6);
    let eye = c
        .rows
        .iter()
        .find(|r| r.metric == "eye_dwell_ratio")
        .unwrap();
    assert_eq!(eye.segment_id, "faces");
    assert_eq!(eye.expected, Some(0.4));
    assert_eq!(eye.recorded, 0.33);
    assert!((eye.deviation.unwrap() - (0.33 - 0.4)).abs() < 1e-12);
    assert_eq!(eye.acceptable_low, Some(0.3));
    assert_eq!(eye.acceptable_high, None);
    assert_eq!(eye.source, "literature");
    assert!(!eye.imputed);
    let latency = c
        .rows
        .iter()
        .find(|r| r.metric == "first_entry_latency_ms")
        .unwrap();
    assert_eq!(latency.target_aoi.as_deref(), Some("Gaze_Target_Right"));
    assert!(latency.imputed);
    // Flat columns from the worst-status row (the atypical latency row).
    assert_eq!(c.metric.as_deref(), Some("first_entry_latency_ms"));
    assert_eq!(c.expected, Some(800.0));
    assert_eq!(c.recorded, Some(10000.0));
    assert_eq!(c.deviation, Some(9200.0));
    assert_eq!(c.baseline_ref.as_deref(), Some("provisional"));
    assert_eq!(c.dwell_social, Some(0.62));
    assert_eq!(c.fixation_count, Some(14));

    let u = &list.items[1];
    assert_eq!(u.clip_id, "untagged");
    assert!(u.rows.is_empty());
    assert_eq!(
        (u.metric.clone(), u.expected, u.recorded, u.deviation),
        (None, None, None, None)
    );
    assert_eq!(u.status, None);
    // Flat members are written as null, rows as [].
    assert_eq!(body["items"][1]["metric"], Value::Null);
    assert_eq!(body["items"][1]["rows"], json!([]));

    let (status, _, _) = send(
        f.router.clone(),
        user_get(&f.user, "/v1/sessions/NOPE/clips"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    f.db.cleanup().await;
}

#[tokio::test]
async fn worst_row_ties_break_on_largest_absolute_deviation() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let mut body = golden();
    // Make the eye-dwell row atypical too: |0.33 - 0.4| < |10000 - 800|, latency still wins;
    // then make a larger-deviation atypical fixation row.
    let rows = &mut body["perVideo"][0]["details"]["rows"];
    rows[2]["status"] = json!("atypical");
    rows[4]["status"] = json!("atypical");
    rows[4]["observed"] = json!(-20000.0);
    ingest_ok(&f.router, &f.dev(), &body).await;
    let (_, _, body) = send(
        f.router.clone(),
        user_get(&f.user, "/v1/sessions/SESS-GOLDEN-0001/clips"),
    )
    .await;
    assert_eq!(body["items"][0]["metric"], "fixation_count");
    assert_eq!(body["items"][0]["deviation"], json!(-20012.0));
    f.db.cleanup().await;
}

#[tokio::test]
async fn org_metrics_count_flags_quality_queue_and_devices() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let now = chrono::Utc::now();
    let ts = |days: i64| {
        (now - chrono::Duration::days(days))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string()
    };
    for (id, days, flag, q) in [
        ("M-1", 1, Some("green"), "optimal"),
        ("M-2", 3, Some("red"), "optimal"),
        ("M-3", 10, None, "acceptable"),
        ("M-4", 40, Some("green"), "degraded"),
    ] {
        let mut b = session(id, &ts(days), flag, q, None);
        if flag.is_none() {
            b["validRatio"] = json!(0.5); // unreliable + low_quality item
        }
        ingest_ok(&f.router, &f.dev(), &b).await;
    }
    sqlx::query("INSERT INTO bundle_adoption (device_id, bundle_version) VALUES ($1, 'b9')")
        .bind(f.device)
        .execute(&f.db.pool)
        .await
        .unwrap();
    let (status, _, body) = send(
        f.router.clone(),
        user_get(&f.user, &format!("/v1/orgs/{}/metrics", f.org)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let m: Metrics = serde_json::from_value(body).unwrap();
    assert_eq!(m.sessions_7d, 2);
    assert_eq!(m.sessions_30d, 3);
    assert_eq!(m.flag_distribution["green"], 2);
    assert_eq!(m.flag_distribution["red"], 1);
    assert_eq!(m.flag_distribution["unreliable"], 1);
    assert_eq!(m.quality_distribution["optimal"], 2);
    assert_eq!(m.quality_distribution["degraded"], 1);
    // flag_red (M-2) + low_quality (M-3).
    assert_eq!(m.review_queue_size, 2);
    assert_eq!(m.devices_active, 1);
    assert_eq!(m.bundle_adoption.len(), 1);
    assert_eq!(m.bundle_adoption[0].bundle_version, "b9");
    assert_eq!(m.bundle_adoption[0].devices, 1);
    f.db.cleanup().await;
}

#[tokio::test]
async fn participants_list_counts_sessions() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.seed_five().await;
    let mut other = session(
        "S-9",
        "2026-10-09T10:00:00.000Z",
        Some("green"),
        "optimal",
        None,
    );
    other["participantToken"] = json!("child-other");
    other["rawPayload"]["participantId"] = json!("child-other");
    ingest_ok(&f.router, &f.dev(), &other).await;
    let (status, _, body) = send(
        f.router.clone(),
        user_get(&f.user, &format!("/v1/orgs/{}/participants", f.org)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: ParticipantPage = serde_json::from_value(body).unwrap();
    assert_eq!(page.total, Some(2));
    let golden_p = page
        .items
        .iter()
        .find(|p| p.participant_token == "child-SESS-GOL")
        .unwrap();
    assert_eq!(golden_p.session_count, 5);
    assert_eq!(
        golden_p.last_session_at.as_deref(),
        Some("2026-10-05T10:00:00.000Z")
    );
    assert_eq!(golden_p.org_id, f.org.to_string());
    let other_p = page
        .items
        .iter()
        .find(|p| p.participant_token == "child-other")
        .unwrap();
    assert_eq!(other_p.session_count, 1);
    f.db.cleanup().await;
}

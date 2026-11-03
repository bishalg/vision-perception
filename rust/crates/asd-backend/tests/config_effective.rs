//! owner: WP-X.2
//!
//! The effective configuration (GOV-6, INV-2; acceptance 14 first half) and the backend
//! rules that read the published profile (INV-1 backend rows): `GET
//! /v1/orgs/{id}/config/effective` answers the org's newest bundle when it has one, else the
//! global published documents, else the bundled defaults; ingest's `low_quality` review rule
//! and derived-flag floor follow the published profile. Needs `DATABASE_URL`.

mod common;
mod ingest_support;
mod research_support;

use asd_backend::config_docs::bundle::BundleSigner;
use asd_contract::api_v1::Role;
use asd_contract::config::ScoringProfile;
use asd_contract::signing;
use http::{Method, StatusCode};
use research_support::*;
use serde_json::{json, Value};

const CATALOG: &str = include_str!("../seed/stimulus_catalog.json");
const BASELINES: &str = include_str!("../seed/normative_baselines.json");

async fn ctx() -> Option<Ctx> {
    let db = common::TestDb::migrated().await?;
    let mut config = common::test_config(common::UNREACHABLE_DB_URL);
    config.bundle_signing = Some(BundleSigner::new(signing::signing_key_from_seed([5u8; 32])));
    let state = asd_backend::app::AppState::new(
        db.pool.clone(),
        std::sync::Arc::new(asd_backend::blob::MemoryBlobStore::new()),
        config,
    );
    Some(Ctx::with_state(db, state).await)
}

async fn publish(c: &Ctx, kind: &str, version: &str, body: &Value) {
    sqlx::query(
        "UPDATE config_documents SET status = 'retired' WHERE kind = $1 AND status = 'published'",
    )
    .bind(kind)
    .execute(&c.db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO config_documents (kind, version, status, body, published_at)
         VALUES ($1, $2, 'published', $3, now())",
    )
    .bind(kind)
    .bind(version)
    .bind(body)
    .execute(&c.db.pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn effective_falls_back_from_org_bundle_to_global_to_defaults() {
    let Some(c) = ctx().await else { return };
    let (_, clinician) = c.token(Role::Clinician).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, viewer) = c.token(Role::Viewer).await;
    let (_, stranger) = c.token_in(Some(c.other), Role::Admin).await;
    let uri = format!("/v1/orgs/{}/config/effective", c.org);

    // Nothing published: the bundled defaults.
    let (s, e) = c.call(Method::GET, &uri, None, &clinician).await;
    assert_eq!(s, StatusCode::OK, "{e}");
    assert_eq!(e["source"], "global");
    assert_eq!(e["bundle_version"], Value::Null);
    assert_eq!(e["scoring_profile"]["version"], "bundled-1.2.0");
    assert_eq!(
        e["scoring_profile"]["body"],
        serde_json::to_value(ScoringProfile::default()).unwrap()
    );
    assert!(e["stimulus_catalog"]["version"]
        .as_str()
        .unwrap()
        .starts_with("bundled-"));

    // Global published documents.
    let mut p = serde_json::to_value(ScoringProfile::default()).unwrap();
    p["trackingQuality"]["drift"]["warnDeg"] = json!(2.5);
    publish(&c, "scoring_profile", "g-1", &p).await;
    publish(
        &c,
        "stimulus_catalog",
        "g-1",
        &serde_json::from_str(CATALOG).unwrap(),
    )
    .await;
    publish(
        &c,
        "normative_baselines",
        "g-1",
        &serde_json::from_str(BASELINES).unwrap(),
    )
    .await;
    let (_, e) = c.call(Method::GET, &uri, None, &clinician).await;
    assert_eq!(e["source"], "global");
    assert_eq!(e["scoring_profile"]["version"], "g-1");
    assert_eq!(
        e["scoring_profile"]["body"]["trackingQuality"]["drift"]["warnDeg"],
        2.5
    );
    assert_eq!(e["normative_baselines"]["version"], "g-1");

    // The org composes its bundle; later global changes do not reach it until it recomposes.
    let (s, b) = c
        .call(Method::POST, "/v1/config/bundles", Some(json!({})), &admin)
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    publish(
        &c,
        "scoring_profile",
        "g-2",
        &serde_json::to_value(ScoringProfile::default()).unwrap(),
    )
    .await;
    let (_, e) = c.call(Method::GET, &uri, None, &clinician).await;
    assert_eq!(e["source"], "org_bundle", "{e}");
    assert_eq!(e["bundle_version"], b["bundle_version"]);
    assert_eq!(e["scoring_profile"]["version"], "g-1");
    // Another org without a bundle sees the global documents.
    let (s, e) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/config/effective", c.other),
            None,
            &stranger,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{e}");
    assert_eq!(e["source"], "global");
    assert_eq!(e["scoring_profile"]["version"], "g-2");

    // ConfigRead: not the viewer; another org's path is a 404.
    let (s, _) = c.call(Method::GET, &uri, None, &viewer).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c.call(Method::GET, &uri, None, &stranger).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

/// INV-1: `auto_review` takes `review.lowQualityValidRatio` and the derived flag takes
/// `lowConfidenceValidRatio` and `flag` from the published profile; without one, today's
/// constants (0.80, 0.60, 0.65/0.35).
#[tokio::test]
async fn ingest_rules_follow_the_published_profile() {
    let Some(c) = Ctx::new().await else { return };
    let (_, device) = common::auth::seed_device(&c.db.pool, c.org).await;
    sqlx::query("UPDATE organizations SET settings = '{\"audit_rate\": 0}' WHERE id = $1")
        .bind(c.org)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let reasons = |id: &'static str| {
        let pool = c.db.pool.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT reason FROM review_items WHERE session_id = $1 ORDER BY reason",
            )
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };
    let flag = |id: &'static str| {
        let pool = c.db.pool.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT attention_flag FROM sessions WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let post = |id: &'static str| {
        let mut v = ingest_support::golden_with_id(id);
        // No device flag: the server derives it.
        v.as_object_mut().unwrap().remove("attentionFlag");
        v
    };

    // Defaults: validRatio 0.93 is not low quality; the flag is derived (green at 0.74).
    ingest_support::ingest_ok(&c.app, &device, &post("S-DEF")).await;
    assert!(reasons("S-DEF").await.is_empty());
    assert_eq!(flag("S-DEF").await, "green");

    let mut p = serde_json::to_value(ScoringProfile::default()).unwrap();
    p["review"]["lowQualityValidRatio"] = json!(0.95);
    p["flag"]["greenFloor"] = json!(0.80);
    publish(&c, "scoring_profile", "strict", &p).await;
    ingest_support::ingest_ok(&c.app, &device, &post("S-STRICT")).await;
    assert_eq!(reasons("S-STRICT").await, ["low_quality"]);
    assert_eq!(flag("S-STRICT").await, "yellow");

    p["lowConfidenceValidRatio"] = json!(0.95);
    publish(&c, "scoring_profile", "stricter", &p).await;
    ingest_support::ingest_ok(&c.app, &device, &post("S-FLOOR")).await;
    assert_eq!(flag("S-FLOOR").await, "unreliable");
    c.db.cleanup().await;
}

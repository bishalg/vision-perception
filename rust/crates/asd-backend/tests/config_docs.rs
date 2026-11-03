//! owner: WP-X.2
//!
//! Config governance for the researcher toolkit (GOV-2, GOV-3, PRM-3; acceptance 13): the
//! draft step takes `ConfigWrite` (owner, admin, researcher), approve and publish stay the
//! vendor's four-eyes steps; publishing a human-authored `scoring_profile` or
//! `normative_baselines` candidate needs a finished analysis run over its body hash (or
//! version), else `409 config-unanalysed`; seeded documents are exempt; drafts are validated
//! by the parameter registry. The end-to-end path: experiment -> run -> promote -> approve ->
//! publish -> a signed bundle that `asd_sync::accept_bundle` accepts with the 1.2.0
//! sections. Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod research_support;

use asd_backend::config_docs::bundle::BundleSigner;
use asd_contract::api_v1::Role;
use asd_contract::config::ScoringProfile;
use asd_contract::signing;
use http::{Method, StatusCode};
use research_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

const CATALOG: &str = include_str!("../seed/stimulus_catalog.json");
const BASELINES: &str = include_str!("../seed/normative_baselines.json");

fn profile() -> Value {
    serde_json::to_value(ScoringProfile::default()).unwrap()
}

async fn ctx_with_signer() -> Option<Ctx> {
    let db = common::TestDb::migrated().await?;
    let mut config = common::test_config(common::UNREACHABLE_DB_URL);
    config.bundle_signing = Some(BundleSigner::new(signing::signing_key_from_seed([9u8; 32])));
    let state = asd_backend::app::AppState::new(
        db.pool.clone(),
        std::sync::Arc::new(asd_backend::blob::MemoryBlobStore::new()),
        config,
    );
    Some(Ctx::with_state(db, state).await)
}

async fn create(
    c: &Ctx,
    kind: &str,
    version: &str,
    body: Value,
    bearer: &str,
) -> (StatusCode, Value) {
    c.call(
        Method::POST,
        &format!("/v1/config/{kind}"),
        Some(json!({"version": version, "body": body})),
        bearer,
    )
    .await
}

async fn step(c: &Ctx, kind: &str, version: &str, step: &str, bearer: &str) -> (StatusCode, Value) {
    c.call(
        Method::POST,
        &format!("/v1/config/{kind}/{version}/{step}"),
        None,
        bearer,
    )
    .await
}

#[tokio::test]
async fn drafts_take_config_write_and_the_registry() {
    let Some(c) = Ctx::new().await else { return };
    for role in [Role::Owner, Role::Admin, Role::Researcher, Role::Superadmin] {
        let (_, t) = c.token(role).await;
        let (s, b) = create(&c, "scoring_profile", &format!("d-{role:?}"), profile(), &t).await;
        assert_eq!(s, StatusCode::CREATED, "{role:?}: {b}");
    }
    for role in [Role::Clinician, Role::Viewer, Role::Reviewer] {
        let (_, t) = c.token(role).await;
        let (s, _) = create(&c, "scoring_profile", "no", profile(), &t).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?}");
    }
    // PRM-3 on create: a registry range and an unknown key name their members.
    let (_, r) = c.token(Role::Researcher).await;
    let mut bad = profile();
    bad["metrics"]["ivtVelocityUnitsPerSec"] = json!(100.0);
    bad["norms"]["typo"] = json!(1);
    let (s, b) = create(&c, "scoring_profile", "bad", bad, &r).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let names: Vec<&str> = b["invalid_params"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["body.metrics.ivtVelocityUnitsPerSec", "body.norms.typo"],
        "{b}"
    );
    // A 1.1.0 body (no 1.2.0 sections) is still a valid draft.
    let mut old = profile();
    for k in ["metrics", "aoi", "ageBands", "norms", "review"] {
        old.as_object_mut().unwrap().remove(k);
    }
    old["schemaVersion"] = json!("1.1.0");
    let (s, b) = create(&c, "scoring_profile", "old", old, &r).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    // Approval stays the vendor's.
    let (s, _) = step(&c, "scoring_profile", "d-Researcher", "approve", &r).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    c.db.cleanup().await;
}

#[tokio::test]
async fn publish_needs_a_finished_run_over_the_candidate() {
    let Some(c) = Ctx::new().await else { return };
    let (_, root) = c.token(Role::Superadmin).await;
    let (_, root2) = c.token(Role::Superadmin).await;
    let (_, root3) = c.token(Role::Superadmin).await;
    let mut p = profile();
    p["flag"]["greenFloor"] = json!(0.7);
    assert_eq!(
        create(&c, "scoring_profile", "p1", p.clone(), &root)
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        step(&c, "scoring_profile", "p1", "approve", &root2).await.0,
        StatusCode::OK
    );
    let (s, b) = step(&c, "scoring_profile", "p1", "publish", &root3).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(common::auth::slug(&b), "config-unanalysed");
    // A run over another body, or a failed run over this one, is no evidence.
    c.finished_run(c.org, None, jcs_hash(&profile()), "b").await;
    let failed = c.finished_run(c.org, None, jcs_hash(&p), "b").await;
    sqlx::query("UPDATE analysis_runs SET status = 'failed' WHERE id = $1")
        .bind(failed)
        .execute(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(
        step(&c, "scoring_profile", "p1", "publish", &root3).await.0,
        StatusCode::CONFLICT
    );
    assert!(c.audit_rows("config.document.publish").await.is_empty());
    // The run over this very body (hash of its RFC 8785 form): 200.
    c.finished_run(c.org, None, jcs_hash(&p), "b").await;
    let (s, b) = step(&c, "scoring_profile", "p1", "publish", &root3).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["status"], "published");

    // Baselines: a run that scored against this version.
    let baselines: Value = serde_json::from_str(BASELINES).unwrap();
    assert_eq!(
        create(&c, "normative_baselines", "b1", baselines, &root)
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        step(&c, "normative_baselines", "b1", "approve", &root2)
            .await
            .0,
        StatusCode::OK
    );
    let (s, b) = step(&c, "normative_baselines", "b1", "publish", &root3).await;
    assert_eq!(common::auth::slug(&b), "config-unanalysed", "{s} {b}");
    c.finished_run(c.org, None, vec![3u8; 32], "b1").await;
    assert_eq!(
        step(&c, "normative_baselines", "b1", "publish", &root3)
            .await
            .0,
        StatusCode::OK
    );

    // The catalog needs no analysis; a seeded (author-less) profile is exempt.
    let catalog: Value = serde_json::from_str(CATALOG).unwrap();
    assert_eq!(
        create(&c, "stimulus_catalog", "c1", catalog, &root).await.0,
        StatusCode::CREATED
    );
    assert_eq!(
        step(&c, "stimulus_catalog", "c1", "approve", &root2)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        step(&c, "stimulus_catalog", "c1", "publish", &root3)
            .await
            .0,
        StatusCode::OK
    );
    let mut seeded = profile();
    seeded["flag"]["greenFloor"] = json!(0.66);
    sqlx::query(
        "INSERT INTO config_documents (kind, version, status, body) VALUES ('scoring_profile', 'seeded', 'draft', $1)",
    )
    .bind(&seeded)
    .execute(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(
        step(&c, "scoring_profile", "seeded", "approve", &root)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        step(&c, "scoring_profile", "seeded", "publish", &root2)
            .await
            .0,
        StatusCode::OK
    );
    c.db.cleanup().await;
}

/// Acceptance 13: experiment -> run -> promote (researcher) -> approve and publish (two
/// superadmins) -> the bundle served next carries the 1.2.0 sections and a device accepts it.
#[tokio::test]
async fn promoted_experiment_reaches_a_device_bundle() {
    let Some(c) = ctx_with_signer().await else {
        return;
    };
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, root) = c.token(Role::Superadmin).await;
    let (_, root2) = c.token(Role::Superadmin).await;
    let (_, admin) = c.token(Role::Admin).await;
    // The catalog and baselines a bundle needs.
    let catalog: Value = serde_json::from_str(CATALOG).unwrap();
    let baselines: Value = serde_json::from_str(BASELINES).unwrap();
    for (kind, body) in [
        ("stimulus_catalog", catalog),
        ("normative_baselines", baselines),
    ] {
        sqlx::query(
            "INSERT INTO config_documents (kind, version, status, body, published_at)
             VALUES ($1, 'seed-1', 'published', $2, now())",
        )
        .bind(kind)
        .bind(body)
        .execute(&c.db.pool)
        .await
        .unwrap();
    }

    let (s, e) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.org),
            Some(json!({"name": "Slower I-VT"})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{e}");
    let id: Uuid = e["id"].as_str().unwrap().parse().unwrap();
    let mut p = e["profile"].clone();
    p["metrics"]["ivtVelocityUnitsPerSec"] = json!(4.0);
    let (s, e) = c
        .patch_if(
            &format!("/v1/experiments/{id}"),
            json!({"profile": p}),
            Some("\"1\""),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{e}");
    // The run (WP-X.3's job) stores the experiment profile's hash.
    c.finished_run(c.org, Some(id), jcs_hash(&e["profile"]), "seed-1")
        .await;
    let (s, doc) = c
        .call(
            Method::POST,
            &format!("/v1/experiments/{id}/promote"),
            Some(json!({"kind": "scoring_profile", "version": "2026.11.03-x"})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{doc}");
    assert_eq!(
        step(&c, "scoring_profile", "2026.11.03-x", "approve", &root)
            .await
            .0,
        StatusCode::OK
    );
    let (s, b) = step(&c, "scoring_profile", "2026.11.03-x", "publish", &root2).await;
    assert_eq!(s, StatusCode::OK, "{b}");

    let (s, summary) = c
        .call(Method::POST, "/v1/config/bundles", Some(json!({})), &admin)
        .await;
    assert_eq!(s, StatusCode::CREATED, "{summary}");
    let (_, device) = common::auth::seed_device(&c.db.pool, c.org).await;
    let req = common::auth::empty(Method::GET, "/v1/config/bundle", Some(&device));
    let (s, _, bundle) = common::send(c.app.clone(), req).await;
    assert_eq!(s, StatusCode::OK, "{bundle}");
    assert_eq!(bundle["scoring_profile"]["schemaVersion"], "1.2.0");
    assert_eq!(
        bundle["scoring_profile"]["metrics"]["ivtVelocityUnitsPerSec"],
        4.0
    );
    assert!(bundle["scoring_profile"]["ageBands"].is_array());
    let vk = signing::signing_key_from_seed([9u8; 32]).verifying_key();
    let ring = [asd_sync::TrustedKey {
        key_id: signing::key_id(&vk),
        public_key_b64: signing::encode_public_key(&vk),
    }];
    let policy = asd_sync::BundlePolicy {
        newest_accepted_version: None,
        now_unix_secs: chrono::Utc::now().timestamp(),
        max_age_secs: None,
    };
    let accepted = asd_sync::accept_bundle(bundle.to_string().as_bytes(), &ring, &policy)
        .expect("a device accepts the published experiment");
    assert_eq!(
        accepted.scoring_profile.metrics.ivt_velocity_units_per_sec,
        4.0
    );
    c.db.cleanup().await;
}

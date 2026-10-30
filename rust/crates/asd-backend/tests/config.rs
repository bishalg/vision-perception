//! owner: WP-B.4
//!
//! CFG-1, CFG-2, CON-6 (server side), HTTP-level with real auth: config documents
//! (`draft -> candidate -> published -> retired`, four-eyes approval), signed bundles
//! (`POST /v1/config/bundles`, `GET /v1/config/bundles[/{version}]`) and device delivery
//! (`GET /v1/config/bundle` with ETag / If-None-Match and adoption rows). Needs
//! `DATABASE_URL`; skipped otherwise (fails with `ASD_REQUIRE_ALL=1`).

mod common;

use std::sync::Arc;

use asd_backend::app::AppState;
use asd_backend::blob::MemoryBlobStore;
use asd_backend::config_docs::bundle::BundleSigner;
use asd_backend::config_docs::seed;
use asd_contract::config::{ConfigBundle, ScoringProfile};
use asd_contract::signing::{self, SigningKey};
use axum::body::Body;
use common::auth::{
    access_token, audit_count, empty, json, seed_device, seed_org, seed_user, slug,
};
use common::{send, TestDb};
use http::{header, Method, Request, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

const CATALOG: &str = include_str!("../seed/stimulus_catalog.json");
const BASELINES: &str = include_str!("../seed/normative_baselines.json");

fn bundle_key() -> SigningKey {
    signing::signing_key_from_seed([7u8; 32])
}

fn catalog() -> Value {
    serde_json::from_str(CATALOG).unwrap()
}
fn baselines() -> Value {
    serde_json::from_str(BASELINES).unwrap()
}
fn profile() -> Value {
    serde_json::to_value(ScoringProfile::default()).unwrap()
}

struct Ctx {
    db: TestDb,
    state: AppState,
    org: Uuid,
    /// org admin A (publisher), org admin B (second pair of eyes), researcher (author)
    admin: String,
    admin_id: Uuid,
    admin2: String,
    researcher: String,
    researcher_id: Uuid,
    viewer: String,
    clinician: String,
    root: String,
}

fn state_for(db: &TestDb, signer: Option<BundleSigner>) -> AppState {
    let mut config = common::test_config(common::UNREACHABLE_DB_URL);
    config.bundle_signing = signer;
    AppState::new(db.pool.clone(), Arc::new(MemoryBlobStore::new()), config)
}

async fn setup_with(signer: Option<BundleSigner>) -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_for(&db, signer);
    let org = seed_org(&db.pool, "Clinic A").await;
    let p = &db.pool;
    use asd_contract::api_v1::Role;
    let admin_id = seed_user(p, Some(org), Role::Admin, "admin@a.test").await;
    let admin2_id = seed_user(p, Some(org), Role::Owner, "owner@a.test").await;
    let researcher_id = seed_user(p, Some(org), Role::Researcher, "res@a.test").await;
    let viewer_id = seed_user(p, Some(org), Role::Viewer, "view@a.test").await;
    let clinician_id = seed_user(p, Some(org), Role::Clinician, "clin@a.test").await;
    let root_id = seed_user(p, None, Role::Superadmin, "root@curv.test").await;
    Some(Ctx {
        admin: access_token(&state, admin_id, Some(org), Role::Admin),
        admin2: access_token(&state, admin2_id, Some(org), Role::Owner),
        researcher: access_token(&state, researcher_id, Some(org), Role::Researcher),
        viewer: access_token(&state, viewer_id, Some(org), Role::Viewer),
        clinician: access_token(&state, clinician_id, Some(org), Role::Clinician),
        root: access_token(&state, root_id, None, Role::Superadmin),
        admin_id,
        researcher_id,
        db,
        state,
        org,
    })
}

async fn setup() -> Option<Ctx> {
    setup_with(Some(BundleSigner::new(bundle_key()))).await
}

impl Ctx {
    async fn call(&self, req: Request<Body>) -> (StatusCode, http::HeaderMap, Value) {
        send(common::router(self.state.clone()), req).await
    }

    async fn post(&self, uri: &str, body: &Value, token: &str) -> (StatusCode, Value) {
        let (s, _, b) = self.call(json(Method::POST, uri, body, Some(token))).await;
        (s, b)
    }

    async fn post_empty(&self, uri: &str, token: &str) -> (StatusCode, Value) {
        let (s, _, b) = self.call(empty(Method::POST, uri, Some(token))).await;
        (s, b)
    }

    async fn get(&self, uri: &str, token: &str) -> (StatusCode, Value) {
        let (s, _, b) = self.call(empty(Method::GET, uri, Some(token))).await;
        (s, b)
    }

    /// Creates `kind/version` as `author`, approved by `approver`, published by `publisher`.
    async fn publish_doc(
        &self,
        kind: &str,
        version: &str,
        body: Value,
        author: &str,
        approver: &str,
        publisher: &str,
    ) {
        let (s, b) = self
            .post(
                &format!("/v1/config/{kind}"),
                &json!({"version": version, "body": body, "notes": "test"}),
                author,
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        let (s, b) = self
            .post_empty(&format!("/v1/config/{kind}/{version}/approve"), approver)
            .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let (s, b) = self
            .post_empty(&format!("/v1/config/{kind}/{version}/publish"), publisher)
            .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }

    /// Publishes catalog, baselines and scoring profile `v` (researcher authors, admin2
    /// approves, admin publishes).
    async fn publish_all(&self, v: &str) {
        let (r, a2, a) = (&self.researcher, &self.admin2, &self.admin);
        self.publish_doc("stimulus_catalog", v, catalog(), r, a2, a)
            .await;
        self.publish_doc("normative_baselines", v, baselines(), r, a2, a)
            .await;
        self.publish_doc("scoring_profile", v, profile(), r, a2, a)
            .await;
    }

    async fn device_get(
        &self,
        token: &str,
        if_none_match: Option<&str>,
    ) -> (StatusCode, http::HeaderMap, Value) {
        let mut b = Request::builder()
            .uri("/v1/config/bundle")
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        if let Some(tag) = if_none_match {
            b = b.header(header::IF_NONE_MATCH, tag);
        }
        self.call(b.body(Body::empty()).unwrap()).await
    }
}

fn verify(bundle: &Value) -> Result<ConfigBundle, signing::SigningError> {
    signing::verify_bundle(&bundle.to_string(), &bundle_key().verifying_key())
}

// ---- documents --------------------------------------------------------------------------

#[tokio::test]
async fn document_routes_need_a_user_and_the_matrix_role() {
    let Some(c) = setup().await else { return };
    let create = json!({"version": "1", "body": profile()});

    let (s, _, b) = c
        .call(empty(Method::GET, "/v1/config/scoring_profile", None))
        .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "{b}");
    // A device token is not a user token.
    let (_, device_token) = seed_device(&c.db.pool, c.org).await;
    let (s, _) = c.get("/v1/config/scoring_profile", &device_token).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    // ConfigRead: owner, admin, clinician, researcher; not viewer.
    let (s, b) = c.get("/v1/config/scoring_profile", &c.viewer).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    let (s, b) = c.get("/v1/config/scoring_profile", &c.clinician).await;
    assert_eq!(s, StatusCode::OK, "{b}");

    // ConfigWrite: owner, admin, researcher; not clinician.
    let (s, _) = c
        .post("/v1/config/scoring_profile", &create, &c.clinician)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = c
        .post("/v1/config/scoring_profile", &create, &c.researcher)
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");

    // ConfigPublish: owner, admin; not researcher.
    let (s, _) = c
        .post_empty("/v1/config/scoring_profile/1/approve", &c.researcher)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c.post_empty("/v1/config/bundles", &c.researcher).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // Unknown kinds are not routes.
    let (s, b) = c.get("/v1/config/feature_flags", &c.admin).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{b}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn create_validates_the_body_per_kind_and_versions_are_unique() {
    let Some(c) = setup().await else { return };
    let r = &c.researcher;

    let (s, b) = c
        .post(
            "/v1/config/stimulus_catalog",
            &json!({"version": "c1", "body": catalog(), "notes": "kit"}),
            r,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(b["kind"], "stimulus_catalog");
    assert_eq!(b["version"], "c1");
    assert_eq!(b["status"], "draft");
    assert_eq!(b["created_by"], c.researcher_id.to_string());
    assert_eq!(b["notes"], "kit");
    assert_eq!(b["published_at"], Value::Null);
    assert_eq!(b["body"], catalog(), "the body is stored as given");

    let (s, b) = c
        .post(
            "/v1/config/stimulus_catalog",
            &json!({"version": "c1", "body": catalog()}),
            r,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug(&b), "config-version-exists");

    // A catalog body under the baselines kind, a broken profile, a bad version string.
    for (kind, body) in [
        (
            "normative_baselines",
            json!({"version": "x", "body": catalog()}),
        ),
        (
            "scoring_profile",
            json!({"version": "x", "body": {"schemaVersion": "1.0.0"}}),
        ),
        ("scoring_profile", json!({"version": "", "body": profile()})),
        (
            "scoring_profile",
            json!({"version": "has space", "body": profile()}),
        ),
        ("scoring_profile", json!({"body": profile()})),
    ] {
        let (s, b) = c.post(&format!("/v1/config/{kind}"), &body, r).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{kind} {body}: {b}");
        assert_eq!(slug(&b), "validation-failed", "{b}");
    }

    // Out-of-range thresholds: yellow floor above green floor.
    let mut p = profile();
    p["flag"]["yellowFloor"] = json!(0.9);
    let (s, b) = c
        .post(
            "/v1/config/scoring_profile",
            &json!({"version": "p", "body": p}),
            r,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");

    // Reads: one document and the list.
    let (s, b) = c.get("/v1/config/stimulus_catalog/c1", r).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["version"], "c1");
    let (s, _) = c.get("/v1/config/stimulus_catalog/nope", r).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, b) = c.get("/v1/config/stimulus_catalog", r).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["items"].as_array().unwrap().len(), 1);
    assert_eq!(b["next_cursor"], Value::Null);
    assert_eq!(audit_count(&c.db.pool, "config.document.create").await, 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn lifecycle_is_draft_approve_publish_with_four_eyes() {
    let Some(c) = setup().await else { return };
    // The admin authors this one, so the admin may neither approve nor publish it.
    let (s, b) = c
        .post(
            "/v1/config/scoring_profile",
            &json!({"version": "p1", "body": profile()}),
            &c.admin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");

    // Publish straight from draft: invalid transition.
    let (s, b) = c
        .post_empty("/v1/config/scoring_profile/p1/publish", &c.admin2)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug(&b), "invalid-transition");

    let (s, b) = c
        .post_empty("/v1/config/scoring_profile/p1/approve", &c.admin)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    assert_eq!(slug(&b), "four-eyes-required");

    let (s, b) = c
        .post_empty("/v1/config/scoring_profile/p1/approve", &c.admin2)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["status"], "candidate");
    // Approving twice is not a transition.
    let (s, b) = c
        .post_empty("/v1/config/scoring_profile/p1/approve", &c.admin2)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");

    let (s, b) = c
        .post_empty("/v1/config/scoring_profile/p1/publish", &c.admin)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    assert_eq!(slug(&b), "four-eyes-required");
    // A superadmin is held to the same rule only for its own documents; here it is a
    // second person.
    let (s, b) = c
        .post_empty("/v1/config/scoring_profile/p1/publish", &c.root)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["status"], "published");
    assert!(b["published_at"].is_string(), "{b}");

    // Publishing p2 retires p1 (one published document per kind).
    c.publish_doc(
        "scoring_profile",
        "p2",
        profile(),
        &c.researcher,
        &c.admin2,
        &c.admin,
    )
    .await;
    let (_, b) = c.get("/v1/config/scoring_profile/p1", &c.admin).await;
    assert_eq!(b["status"], "retired");
    let (_, b) = c
        .get("/v1/config/scoring_profile?status=published", &c.admin)
        .await;
    let items = b["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{b}");
    assert_eq!(items[0]["version"], "p2");

    // Retire: anything not yet retired; retiring twice is a conflict.
    let (s, b) = c
        .post_empty("/v1/config/scoring_profile/p2/retire", &c.admin)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["status"], "retired");
    let (s, _) = c
        .post_empty("/v1/config/scoring_profile/p2/retire", &c.admin)
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = c
        .post_empty("/v1/config/scoring_profile/p2/publish", &c.admin2)
        .await;
    assert_eq!(
        s,
        StatusCode::CONFLICT,
        "a retired document is never re-published"
    );

    // One audit row per successful mutation (AUTH-5), none for refused ones.
    let p = &c.db.pool;
    assert_eq!(audit_count(p, "config.document.create").await, 2);
    assert_eq!(audit_count(p, "config.document.approve").await, 2);
    assert_eq!(audit_count(p, "config.document.publish").await, 2);
    assert_eq!(audit_count(p, "config.document.retire").await, 1);
    let approver: Option<Uuid> = sqlx::query_scalar(
        "SELECT approved_by FROM config_documents WHERE kind = 'scoring_profile' AND version = 'p2'",
    )
    .fetch_one(p)
    .await
    .unwrap();
    assert!(approver.is_some());
    c.db.cleanup().await;
}

#[tokio::test]
async fn seeded_drafts_need_two_different_people_and_are_never_published_by_seeding() {
    let Some(c) = setup().await else { return };
    assert_eq!(seed::seed_if_empty(&c.db.pool).await.unwrap(), 3);
    assert_eq!(
        seed::seed_if_empty(&c.db.pool).await.unwrap(),
        0,
        "only on first boot"
    );
    let rows: Vec<(String, String, Option<Uuid>, Value)> =
        sqlx::query_as("SELECT kind, status, created_by, body FROM config_documents ORDER BY kind")
            .fetch_all(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 3);
    for (kind, status, created_by, body) in &rows {
        assert_eq!(status, "draft", "{kind}: CFG-2 never auto-publishes");
        assert_eq!(*created_by, None);
        let expected = match kind.as_str() {
            "stimulus_catalog" => catalog(),
            "normative_baselines" => baselines(),
            "scoring_profile" => profile(),
            other => panic!("unexpected kind {other}"),
        };
        assert_eq!(body, &expected, "{kind}");
    }
    assert_eq!(audit_count(&c.db.pool, "config.document.seed").await, 1);

    // The device sees nothing until a bundle is composed from published documents.
    let (_, device_token) = seed_device(&c.db.pool, c.org).await;
    let (s, _, _) = c.device_get(&device_token, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, b) = c.post_empty("/v1/config/bundles", &c.admin).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug(&b), "config-not-published");

    // System-authored: approver and publisher must be two people.
    let (_, b) = c.get("/v1/config/scoring_profile", &c.admin).await;
    let version = b["items"][0]["version"].as_str().unwrap().to_owned();
    let base = format!("/v1/config/scoring_profile/{version}");
    let (s, b) = c.post_empty(&format!("{base}/approve"), &c.admin).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = c.post_empty(&format!("{base}/publish"), &c.admin).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    assert_eq!(slug(&b), "four-eyes-required");
    let (s, b) = c.post_empty(&format!("{base}/publish"), &c.admin2).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn seeding_skips_when_any_document_exists() {
    let Some(c) = setup().await else { return };
    let (s, _) = c
        .post(
            "/v1/config/scoring_profile",
            &json!({"version": "mine", "body": profile()}),
            &c.researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(seed::seed_if_empty(&c.db.pool).await.unwrap(), 0);
    assert_eq!(audit_count(&c.db.pool, "config.document.seed").await, 0);
    c.db.cleanup().await;
}

// ---- bundles ----------------------------------------------------------------------------

#[tokio::test]
async fn compose_signs_and_the_device_gets_it_with_etag_304_and_adoption() {
    let Some(c) = setup().await else { return };
    c.publish_all("1").await;

    let (s, headers, summary) = c
        .call(json(
            Method::POST,
            "/v1/config/bundles",
            &json!({"feature_flags": {"newOnboarding": true}}),
            Some(&c.admin),
        ))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{summary}");
    let version = summary["bundle_version"].as_str().unwrap().to_owned();
    let today = chrono::Utc::now().format("%Y.%m.%d").to_string();
    assert_eq!(version, format!("{today}-1"));
    assert_eq!(summary["catalog_version"], "1");
    assert_eq!(summary["baselines_version"], "1");
    assert_eq!(summary["scoring_profile_version"], "1");
    let key_id = signing::key_id(&bundle_key().verifying_key());
    assert_eq!(summary["key_id"], key_id.as_str());
    assert_eq!(headers[header::ETAG], format!("\"{version}\"").as_str());
    assert_eq!(audit_count(&c.db.pool, "config.bundle.publish").await, 1);

    let (device, token) = seed_device(&c.db.pool, c.org).await;
    let (s, headers, bundle) = c.device_get(&token, None).await;
    assert_eq!(s, StatusCode::OK, "{bundle}");
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    assert_eq!(etag, format!("\"{version}\""));
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    let decoded = verify(&bundle).expect("the served bundle verifies (CON-6)");
    assert_eq!(decoded.bundle_version, version);
    assert_eq!(decoded.key_id, key_id);
    assert_eq!(decoded.scoring_profile, ScoringProfile::default());
    assert_eq!(decoded.feature_flags.get("newOnboarding"), Some(&true));
    assert!(bundle["signature"]
        .as_str()
        .unwrap()
        .starts_with("ed25519:"));

    // Flip one value: the signature no longer matches.
    let mut tampered = bundle.clone();
    tampered["scoring_profile"]["flag"]["greenFloor"] = json!(0.66);
    assert_eq!(verify(&tampered), Err(signing::SigningError::BadSignature));

    // Same ETag -> 304, no body; a list and a weak tag also match; another tag -> 200.
    for tag in [
        etag.clone(),
        format!("W/{etag}"),
        format!("\"other\", {etag}"),
        "*".to_owned(),
    ] {
        let (s, headers, body) = c.device_get(&token, Some(&tag)).await;
        assert_eq!(s, StatusCode::NOT_MODIFIED, "{tag}");
        assert_eq!(body, Value::Null);
        assert_eq!(headers[header::ETAG], etag.as_str());
    }
    let (s, _, _) = c.device_get(&token, Some("\"2000.01.01-1\"")).await;
    assert_eq!(s, StatusCode::OK);

    // Adoption: one row per (device, version) however often it is fetched.
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT device_id, bundle_version FROM bundle_adoption")
            .fetch_all(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(rows, vec![(device, version.clone())]);

    // A newer bundle: the old ETag now gets the new body.
    let (s, b) = c.post_empty("/v1/config/bundles", &c.admin).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(b["bundle_version"], format!("{today}-2"));
    let (s, headers, bundle) = c.device_get(&token, Some(&etag)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(bundle["bundle_version"], format!("{today}-2"));
    assert_ne!(headers[header::ETAG], etag.as_str());
    verify(&bundle).unwrap();
    assert!(bundle["feature_flags"].as_object().unwrap().is_empty());

    // Console reads: list (newest first) and one bundle as served.
    let (s, b) = c.get("/v1/config/bundles", &c.clinician).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let items = b["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["bundle_version"], format!("{today}-2"));
    let (s, b) = c
        .get(&format!("/v1/config/bundles/{version}"), &c.researcher)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    verify(&b).unwrap();
    let (s, _) = c.get("/v1/config/bundles/nope", &c.admin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c.get("/v1/config/bundles", &c.viewer).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_device_route_needs_a_live_device_token() {
    let Some(c) = setup().await else { return };
    c.publish_all("1").await;
    let (s, b) = c.post_empty("/v1/config/bundles", &c.admin).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");

    let (s, _, _) = c.call(empty(Method::GET, "/v1/config/bundle", None)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // A user JWT is not a device token.
    let (s, _, _) = c.device_get(&c.admin, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (device, token) = seed_device(&c.db.pool, c.org).await;
    sqlx::query("UPDATE devices SET revoked_at = now() WHERE id = $1")
        .bind(device)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, _, _) = c.device_get(&token, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    c.db.cleanup().await;
}

#[tokio::test]
async fn org_bundles_override_the_global_default_and_stay_in_their_org() {
    let Some(c) = setup().await else { return };
    c.publish_all("1").await;
    let other = seed_org(&c.db.pool, "Clinic B").await;
    let other_admin_id = seed_user(
        &c.db.pool,
        Some(other),
        asd_contract::api_v1::Role::Admin,
        "admin@b.test",
    )
    .await;
    let other_admin = access_token(
        &c.state,
        other_admin_id,
        Some(other),
        asd_contract::api_v1::Role::Admin,
    );

    // Superadmin: global default (no org_id).
    let (s, global) = c.post_empty("/v1/config/bundles", &c.root).await;
    assert_eq!(s, StatusCode::CREATED, "{global}");
    // Org admin: always its own org; naming another org is a 404.
    let (s, b) = c
        .post(
            "/v1/config/bundles",
            &json!({"org_id": other.to_string()}),
            &c.admin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{b}");
    let (s, org_a) = c.post_empty("/v1/config/bundles", &c.admin).await;
    assert_eq!(s, StatusCode::CREATED, "{org_a}");
    // Superadmin for a named org.
    let (s, b) = c
        .post(
            "/v1/config/bundles",
            &json!({"org_id": Uuid::new_v4().to_string()}),
            &c.root,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "unknown org: {b}");

    let (_, token_a) = seed_device(&c.db.pool, c.org).await;
    let (_, token_b) = seed_device(&c.db.pool, other).await;
    let (_, _, a) = c.device_get(&token_a, None).await;
    assert_eq!(a["bundle_version"], org_a["bundle_version"]);
    let (_, _, b) = c.device_get(&token_b, None).await;
    assert_eq!(b["bundle_version"], global["bundle_version"]);

    // Clinic B sees the global bundle and not Clinic A's.
    let (s, list) = c.get("/v1/config/bundles", &other_admin).await;
    assert_eq!(s, StatusCode::OK);
    let versions: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["bundle_version"].as_str().unwrap())
        .collect();
    assert_eq!(versions, vec![global["bundle_version"].as_str().unwrap()]);
    let (s, _) = c
        .get(
            &format!(
                "/v1/config/bundles/{}",
                org_a["bundle_version"].as_str().unwrap()
            ),
            &other_admin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, list) = c.get("/v1/config/bundles", &c.root).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    c.db.cleanup().await;
}

#[tokio::test]
async fn compose_needs_a_signing_key() {
    let Some(c) = setup_with(None).await else {
        return;
    };
    c.publish_all("1").await;
    let (s, b) = c.post_empty("/v1/config/bundles", &c.admin).await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE, "{b}");
    assert_eq!(slug(&b), "bundle-signing-unavailable");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM config_bundles")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
    assert_eq!(audit_count(&c.db.pool, "config.bundle.publish").await, 0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_stored_bundle_that_fails_verification_is_never_served() {
    let Some(c) = setup().await else { return };
    c.publish_all("1").await;
    let (s, b) = c.post_empty("/v1/config/bundles", &c.admin).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    // Someone edits the stored body behind the API.
    sqlx::query(
        "UPDATE config_bundles SET body = jsonb_set(body, '{scoring_profile,flag,greenFloor}', '0.99')",
    )
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (_, token) = seed_device(&c.db.pool, c.org).await;
    let (s, _, body) = c.device_get(&token, None).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(body.get("signature").is_none(), "no bundle in the body");

    // An unsigned row is refused the same way.
    sqlx::query("UPDATE config_bundles SET body = body - 'signature'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, _, _) = c.device_get(&token, None).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    c.db.cleanup().await;
}

#[tokio::test]
async fn concurrent_composes_get_distinct_versions() {
    let Some(c) = setup().await else { return };
    c.publish_all("1").await;
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let router = common::router(c.state.clone());
        let req = empty(Method::POST, "/v1/config/bundles", Some(&c.admin));
        tasks.push(tokio::spawn(async move { send(router, req).await }));
    }
    let mut versions = Vec::new();
    for t in tasks {
        let (s, _, b) = t.await.unwrap();
        assert_eq!(s, StatusCode::CREATED, "{b}");
        versions.push(b["bundle_version"].as_str().unwrap().to_owned());
    }
    versions.sort();
    versions.dedup();
    assert_eq!(versions.len(), 6, "{versions:?}");
    let _ = c.admin_id;
    c.db.cleanup().await;
}

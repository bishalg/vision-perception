//! owner: WP-B.2
//!
//! CON-10 (register response shape), CON-11 (enrol codes with `max_uses`/`expires_at`,
//! re-registration with `device_id` rotates without consuming a use, 410 semantics),
//! AUTH-3 (`dvc_` tokens, SHA-256 at rest, bound to device and org, revocable), and the
//! console device routes (`GET /orgs/{id}/devices`, `POST .../enrol-code`,
//! `POST /devices/{id}/revoke`). Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::app::AppState;
use asd_backend::auth::device_token;
use asd_contract::api_v1::Role;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
    org: Uuid,
    admin: String,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let admin_id = seed_user(&db.pool, Some(org), Role::Admin, "admin@example.org").await;
    let admin = access_token(&state, admin_id, Some(org), Role::Admin);
    Some(Ctx {
        db,
        state,
        app,
        org,
        admin,
    })
}

impl Ctx {
    async fn enrol_code(&self, body: Value) -> (StatusCode, Value) {
        let (s, _, b) = send(
            self.app.clone(),
            json(
                Method::POST,
                &format!("/v1/orgs/{}/devices/enrol-code", self.org),
                &body,
                Some(&self.admin),
            ),
        )
        .await;
        (s, b)
    }

    async fn register(&self, body: Value) -> (StatusCode, Value) {
        let (s, _, b) = send(
            self.app.clone(),
            json(Method::POST, "/v1/devices/register", &body, None),
        )
        .await;
        (s, b)
    }

    async fn uses(&self, code: &str) -> i32 {
        sqlx::query_scalar("SELECT uses FROM enrol_codes WHERE code_hash = $1")
            .bind(asd_backend::auth::enrol_code_hash(code).as_slice())
            .fetch_one(&self.db.pool)
            .await
            .unwrap()
    }
}

fn reg(code: &str) -> Value {
    json!({"enrol_code": code, "platform": "iOS", "app_version": "1.4.0"})
}

fn reg_with_device(code: &str, device_id: &str) -> Value {
    json!({"enrol_code": code, "platform": "iOS", "app_version": "1.4.1", "device_id": device_id})
}

#[tokio::test]
async fn enrol_code_defaults_to_one_use_and_seven_days_and_is_stored_hashed() {
    let Some(c) = setup().await else { return };
    let (status, body) = c.enrol_code(json!({})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["max_uses"], 1);
    let code = body["code"].as_str().unwrap();
    assert!(code.len() >= 12, "enough entropy: {code}");
    let expires = chrono::DateTime::parse_from_rfc3339(body["expires_at"].as_str().unwrap())
        .unwrap()
        .with_timezone(&chrono::Utc);
    let days = (expires - chrono::Utc::now()).num_minutes() as f64 / 1440.0;
    assert!((days - 7.0).abs() < 0.01, "{days}");

    let (stored_hash, org): (Vec<u8>, Uuid) =
        sqlx::query_as("SELECT code_hash, org_id FROM enrol_codes WHERE id = $1")
            .bind(Uuid::parse_str(body["id"].as_str().unwrap()).unwrap())
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(org, c.org);
    assert_eq!(
        stored_hash,
        asd_backend::auth::enrol_code_hash(code).to_vec()
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn enrol_code_options_are_validated() {
    let Some(c) = setup().await else { return };
    let (status, body) = c
        .enrol_code(json!({"max_uses": 5, "expires_in_days": 30, "label": "Ward 3 iPads"}))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["max_uses"], 5);

    for bad in [
        json!({"max_uses": 0}),
        json!({"max_uses": 10001}),
        json!({"expires_in_days": 0}),
        json!({"expires_in_days": 91}),
        json!({"label": "x".repeat(201)}),
    ] {
        let (status, body) = c.enrol_code(bad.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
        assert_eq!(slug(&body), "validation-failed");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn register_returns_the_con10_shape_and_stores_the_token_hashed() {
    let Some(c) = setup().await else { return };
    let (_, code) = c.enrol_code(json!({"label": "iPad 1"})).await;
    let code = code["code"].as_str().unwrap().to_owned();

    let (status, body) = c.register(reg(&code)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "config_bundle_version",
            "device_id",
            "device_token",
            "org_id",
            "status"
        ]
    );
    assert_eq!(body["status"], "registered");
    assert_eq!(body["org_id"], c.org.to_string());
    assert_eq!(body["config_bundle_version"], Value::Null);
    let device_id = Uuid::parse_str(body["device_id"].as_str().unwrap()).unwrap();
    let token = body["device_token"].as_str().unwrap();
    let raw = token.strip_prefix("dvc_").expect("dvc_ prefix");
    assert_eq!(URL_SAFE_NO_PAD.decode(raw).unwrap().len(), 32);

    let (hash, label, platform, app_version, enrol_code_id): (
        Vec<u8>,
        Option<String>,
        String,
        String,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT token_hash, label, platform, app_version, enrol_code_id FROM devices WHERE id = $1",
    )
    .bind(device_id)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(hash, Sha256::digest(token.as_bytes()).to_vec());
    assert_eq!(label.as_deref(), Some("iPad 1"));
    assert_eq!((platform.as_str(), app_version.as_str()), ("iOS", "1.4.0"));
    assert!(enrol_code_id.is_some());
    assert_eq!(c.uses(&code).await, 1);

    let principal = device_token::authenticate(&c.db.pool, token)
        .await
        .unwrap()
        .expect("token authenticates");
    assert_eq!((principal.device_id, principal.org_id), (device_id, c.org));
    c.db.cleanup().await;
}

#[tokio::test]
async fn acceptance_6_consumed_code_is_410_without_device_id_and_rotates_with_it() {
    let Some(c) = setup().await else { return };
    let (_, code) = c.enrol_code(json!({})).await;
    let code = code["code"].as_str().unwrap().to_owned();
    let (_, first) = c.register(reg(&code)).await;
    let device_id = first["device_id"].as_str().unwrap().to_owned();
    let old_token = first["device_token"].as_str().unwrap().to_owned();

    let (status, body) = c.register(reg(&code)).await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert_eq!(slug(&body), "enrol-code-exhausted");
    assert_eq!(c.uses(&code).await, 1);

    let (status, body) = c.register(reg_with_device(&code, &device_id)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "rotated");
    assert_eq!(body["device_id"], device_id);
    assert_eq!(body["org_id"], c.org.to_string());
    let new_token = body["device_token"].as_str().unwrap();
    assert_ne!(new_token, old_token);
    assert_eq!(c.uses(&code).await, 1, "rotation consumes no use");

    assert!(device_token::authenticate(&c.db.pool, &old_token)
        .await
        .unwrap()
        .is_none());
    assert!(device_token::authenticate(&c.db.pool, new_token)
        .await
        .unwrap()
        .is_some());
    let app_version: String = sqlx::query_scalar("SELECT app_version FROM devices WHERE id = $1")
        .bind(Uuid::parse_str(&device_id).unwrap())
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(app_version, "1.4.1");
    let devices: i64 = sqlx::query_scalar("SELECT count(*) FROM devices")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(devices, 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn device_id_only_rotates_a_live_device_enrolled_by_that_code() {
    let Some(c) = setup().await else { return };
    let code_a = seed_enrol_code(&c.db.pool, c.org, 1).await;
    let code_b = seed_enrol_code(&c.db.pool, c.org, 1).await;
    let (_, a) = c.register(reg(&code_a)).await;
    let (_, b) = c.register(reg(&code_b)).await;
    let a_id = a["device_id"].as_str().unwrap().to_owned();
    let b_id = b["device_id"].as_str().unwrap().to_owned();

    // Device B with code A: not enrolled by A, and A is used up.
    let (status, body) = c.register(reg_with_device(&code_a, &b_id)).await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    // Unknown device id: same.
    let (status, _) = c
        .register(reg_with_device(&code_a, &Uuid::new_v4().to_string()))
        .await;
    assert_eq!(status, StatusCode::GONE);

    // A revoked device cannot rotate itself back to life.
    let (status, _, _) = send(
        c.app.clone(),
        empty(
            Method::POST,
            &format!("/v1/devices/{a_id}/revoke"),
            Some(&c.admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = c.register(reg_with_device(&code_a, &a_id)).await;
    assert_eq!(status, StatusCode::GONE);

    // A malformed device id is a validation error.
    let (status, body) = c.register(reg_with_device(&code_a, "not-a-uuid")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(slug(&body), "validation-failed");
    c.db.cleanup().await;
}

#[tokio::test]
async fn multi_use_codes_count_uses_and_new_devices_without_device_id() {
    let Some(c) = setup().await else { return };
    let (_, code) = c.enrol_code(json!({"max_uses": 2})).await;
    let code = code["code"].as_str().unwrap().to_owned();
    let (s1, d1) = c.register(reg(&code)).await;
    let (s2, d2) = c.register(reg(&code)).await;
    let (s3, b3) = c.register(reg(&code)).await;
    assert_eq!((s1, s2), (StatusCode::CREATED, StatusCode::CREATED));
    assert_ne!(d1["device_id"], d2["device_id"]);
    assert_eq!(s3, StatusCode::GONE, "{b3}");
    assert_eq!(slug(&b3), "enrol-code-exhausted");
    assert_eq!(c.uses(&code).await, 2);
    c.db.cleanup().await;
}

#[tokio::test]
async fn expired_revoked_and_unknown_codes() {
    let Some(c) = setup().await else { return };
    let expired = seed_enrol_code(&c.db.pool, c.org, 1).await;
    let revoked = seed_enrol_code(&c.db.pool, c.org, 1).await;
    sqlx::query(
        "UPDATE enrol_codes SET expires_at = now() - interval '1 minute' WHERE code_hash = $1",
    )
    .bind(asd_backend::auth::enrol_code_hash(&expired).as_slice())
    .execute(&c.db.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE enrol_codes SET revoked_at = now() WHERE code_hash = $1")
        .bind(asd_backend::auth::enrol_code_hash(&revoked).as_slice())
        .execute(&c.db.pool)
        .await
        .unwrap();

    let (status, body) = c.register(reg(&expired)).await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert_eq!(slug(&body), "enrol-code-expired");
    let (status, body) = c.register(reg(&revoked)).await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert_eq!(slug(&body), "enrol-code-revoked");
    let (status, body) = c.register(reg("NOPE-NOPE-NOPE")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn rotation_still_works_after_the_code_expires_but_not_after_it_is_revoked() {
    // iOS keeps the code and re-registers on 401 for the life of the install (F6); the
    // 7-day expiry limits new enrolments, not re-registration of an enrolled device.
    let Some(c) = setup().await else { return };
    let code = seed_enrol_code(&c.db.pool, c.org, 1).await;
    let (_, first) = c.register(reg(&code)).await;
    let device_id = first["device_id"].as_str().unwrap().to_owned();
    let hash = asd_backend::auth::enrol_code_hash(&code);
    sqlx::query(
        "UPDATE enrol_codes SET expires_at = now() - interval '1 day' WHERE code_hash = $1",
    )
    .bind(hash.as_slice())
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (status, body) = c.register(reg_with_device(&code, &device_id)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "rotated");

    sqlx::query("UPDATE enrol_codes SET revoked_at = now() WHERE code_hash = $1")
        .bind(hash.as_slice())
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (status, body) = c.register(reg_with_device(&code, &device_id)).await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert_eq!(slug(&body), "enrol-code-revoked");
    c.db.cleanup().await;
}

#[tokio::test]
async fn codes_are_normalised_case_spaces_and_dashes() {
    let Some(c) = setup().await else { return };
    let (_, body) = c.enrol_code(json!({})).await;
    let code = body["code"].as_str().unwrap();
    let typed = format!("  {} ", code.replace('-', " ").to_lowercase());
    let (status, body) = c.register(reg(&typed)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn parallel_registrations_never_exceed_max_uses() {
    let Some(c) = setup().await else { return };
    let code = seed_enrol_code(&c.db.pool, c.org, 1).await;
    let tasks: Vec<_> = (0..10)
        .map(|_| {
            let app = c.app.clone();
            let body = reg(&code);
            tokio::spawn(async move {
                send(app, json(Method::POST, "/v1/devices/register", &body, None))
                    .await
                    .0
            })
        })
        .collect();
    let mut created = 0;
    for t in tasks {
        let s = t.await.unwrap();
        assert!(s == StatusCode::CREATED || s == StatusCode::GONE, "{s}");
        created += usize::from(s == StatusCode::CREATED);
    }
    assert_eq!(created, 1);
    assert_eq!(c.uses(&code).await, 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn register_validates_the_body() {
    let Some(c) = setup().await else { return };
    let code = seed_enrol_code(&c.db.pool, c.org, 5).await;
    for bad in [
        json!({"platform": "iOS", "app_version": "1"}),
        json!({"enrol_code": "", "platform": "iOS", "app_version": "1"}),
        json!({"enrol_code": code, "platform": "", "app_version": "1"}),
        json!({"enrol_code": code, "platform": "iOS", "app_version": "1".repeat(65)}),
    ] {
        let (status, body) = c.register(bad.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
        assert_eq!(slug(&body), "validation-failed");
    }
    assert_eq!(c.uses(&code).await, 0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn register_reports_the_latest_bundle_for_the_org() {
    let Some(c) = setup().await else { return };
    let docs: Vec<Uuid> = {
        let mut ids = Vec::new();
        for kind in ["stimulus_catalog", "normative_baselines", "scoring_profile"] {
            ids.push(
                sqlx::query_scalar(
                    "INSERT INTO config_documents (kind, version, status, body) \
                     VALUES ($1, '1', 'published', '{}') RETURNING id",
                )
                .bind(kind)
                .fetch_one(&c.db.pool)
                .await
                .unwrap(),
            );
        }
        ids
    };
    for (org, version, age) in [
        (None, "global-1", "3 days"),
        (Some(c.org), "org-1", "2 days"),
        (Some(c.org), "org-2", "1 day"),
    ] {
        sqlx::query(
            "INSERT INTO config_bundles (org_id, catalog_id, baselines_id, scoring_id, \
               bundle_version, body, signature, published_at) \
             VALUES ($1, $2, $3, $4, $5, '{}', 'ed25519:x', now() - $6::interval)",
        )
        .bind(org)
        .bind(docs[0])
        .bind(docs[1])
        .bind(docs[2])
        .bind(version)
        .bind(age)
        .execute(&c.db.pool)
        .await
        .unwrap();
    }
    let code = seed_enrol_code(&c.db.pool, c.org, 1).await;
    let (status, body) = c.register(reg(&code)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["config_bundle_version"], "org-2");

    // An org without its own bundle gets the global default.
    let other = seed_org(&c.db.pool, "Clinic B").await;
    let code = seed_enrol_code(&c.db.pool, other, 1).await;
    let (_, body) = c.register(reg(&code)).await;
    assert_eq!(body["config_bundle_version"], "global-1");
    c.db.cleanup().await;
}

#[tokio::test]
async fn revoke_kills_the_token_and_is_org_scoped() {
    let Some(c) = setup().await else { return };
    let (device, token) = seed_device(&c.db.pool, c.org).await;
    let other_org = seed_org(&c.db.pool, "Clinic B").await;
    let (foreign, _) = seed_device(&c.db.pool, other_org).await;

    let (status, _, body) = send(
        c.app.clone(),
        empty(
            Method::POST,
            &format!("/v1/devices/{foreign}/revoke"),
            Some(&c.admin),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another org's device: {body}"
    );

    let clinician = seed_user(&c.db.pool, Some(c.org), Role::Clinician, "c@example.org").await;
    let clinician = access_token(&c.state, clinician, Some(c.org), Role::Clinician);
    let (status, _, _) = send(
        c.app.clone(),
        empty(
            Method::POST,
            &format!("/v1/devices/{device}/revoke"),
            Some(&clinician),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _, body) = send(
        c.app.clone(),
        empty(
            Method::POST,
            &format!("/v1/devices/{device}/revoke"),
            Some(&c.admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["id"], device.to_string());
    assert!(body["revoked_at"].is_string());
    assert!(device_token::authenticate(&c.db.pool, &token)
        .await
        .unwrap()
        .is_none());

    // Idempotent: a second revoke keeps the first timestamp and writes no audit row.
    let audits = audit_count(&c.db.pool, "device.revoke").await;
    let (status, _, again) = send(
        c.app.clone(),
        empty(
            Method::POST,
            &format!("/v1/devices/{device}/revoke"),
            Some(&c.admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["revoked_at"], body["revoked_at"]);
    assert_eq!(audit_count(&c.db.pool, "device.revoke").await, audits);

    let (status, _, _) = send(
        c.app.clone(),
        empty(
            Method::POST,
            "/v1/devices/not-a-uuid/revoke",
            Some(&c.admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn device_list_is_org_scoped_paginated_and_shows_bundle_and_last_seen() {
    let Some(c) = setup().await else { return };
    let mut mine = Vec::new();
    for _ in 0..3 {
        mine.push(seed_device(&c.db.pool, c.org).await.0);
    }
    let other = seed_org(&c.db.pool, "Clinic B").await;
    seed_device(&c.db.pool, other).await;

    let uri = format!("/v1/orgs/{}/devices?limit=2", c.org);
    let (status, _, page1) = send(c.app.clone(), empty(Method::GET, &uri, Some(&c.admin))).await;
    assert_eq!(status, StatusCode::OK, "{page1}");
    assert_eq!(page1["items"].as_array().unwrap().len(), 2);
    let cursor = page1["next_cursor"].as_str().expect("more pages");
    let uri = format!("/v1/orgs/{}/devices?limit=2&cursor={cursor}", c.org);
    let (status, _, page2) = send(c.app.clone(), empty(Method::GET, &uri, Some(&c.admin))).await;
    assert_eq!(status, StatusCode::OK, "{page2}");
    assert_eq!(page2["items"].as_array().unwrap().len(), 1);
    assert_eq!(page2["next_cursor"], Value::Null);

    let mut seen: Vec<String> = page1["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(page2["items"].as_array().unwrap())
        .map(|d| d["id"].as_str().unwrap().to_owned())
        .collect();
    seen.sort();
    let mut want: Vec<String> = mine.iter().map(Uuid::to_string).collect();
    want.sort();
    assert_eq!(seen, want, "only this org's devices, each once");
    let item = &page1["items"][0];
    for key in [
        "id",
        "org_id",
        "platform",
        "app_version",
        "label",
        "enrolled_at",
        "revoked_at",
        "last_seen_at",
        "config_bundle_version",
    ] {
        assert!(item.get(key).is_some(), "{key} in {item}");
    }

    let (status, _, body) = send(
        c.app.clone(),
        empty(
            Method::GET,
            &format!("/v1/orgs/{}/devices?limit=101", c.org),
            Some(&c.admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _, _) = send(
        c.app.clone(),
        empty(
            Method::GET,
            &format!("/v1/orgs/{}/devices?cursor=%%%", c.org),
            Some(&c.admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    c.db.cleanup().await;
}

#[tokio::test]
async fn device_extractor_accepts_only_live_dvc_tokens() {
    let Some(c) = setup().await else { return };
    let (_, token) = seed_device(&c.db.pool, c.org).await;
    assert!(device_token::authenticate(&c.db.pool, &token)
        .await
        .unwrap()
        .is_some());
    for bad in [
        "",
        "dvc_",
        "dvc_short",
        token.trim_start_matches("dvc_"),
        &format!("{token}x"),
        "eyJhbGciOiJFZERTQSJ9.e30.c2ln",
    ] {
        assert!(
            device_token::authenticate(&c.db.pool, bad)
                .await
                .unwrap()
                .is_none(),
            "{bad}"
        );
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn register_is_rate_limited_per_ip() {
    let Some(c) = setup().await else { return };
    for i in 0..10 {
        let (status, _, _) = send(
            c.app.clone(),
            from_ip(
                json(
                    Method::POST,
                    "/v1/devices/register",
                    &reg("NOPE-NOPE-NOPE"),
                    None,
                ),
                "198.51.100.4",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "attempt {i}");
    }
    let (status, headers, _) = send(
        c.app.clone(),
        from_ip(
            json(
                Method::POST,
                "/v1/devices/register",
                &reg("NOPE-NOPE-NOPE"),
                None,
            ),
            "198.51.100.4",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(headers.contains_key(http::header::RETRY_AFTER));
    c.db.cleanup().await;
}

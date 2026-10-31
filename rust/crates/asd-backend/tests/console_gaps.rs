//! owner: WP-B.2 (console gaps)
//!
//! `GET /v1/orgs` (org switcher), `settings` on `GET /v1/orgs/{id}`, and `user_id` /
//! `totp_enrolled` on `GET /v1/users/me`. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::app::AppState;
use asd_contract::api_v1::Role;
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    Some(Ctx { db, state, app })
}

impl Ctx {
    async fn token(&self, org: Option<Uuid>, role: Role) -> (Uuid, String) {
        let email = format!("{}-{}@example.org", role_str(role), Uuid::new_v4().simple());
        let id = seed_user(&self.db.pool, org, role, &email).await;
        (id, access_token(&self.state, id, org, role))
    }

    async fn get(&self, uri: &str, bearer: Option<&str>) -> (StatusCode, Value) {
        let (s, _, b) = send(self.app.clone(), empty(Method::GET, uri, bearer)).await;
        (s, b)
    }
}

#[tokio::test]
async fn org_list_is_own_org_for_users_and_all_paginated_for_superadmin() {
    let Some(c) = setup().await else { return };
    let a = seed_org(&c.db.pool, "Clinic A").await;
    let b = seed_org(&c.db.pool, "Clinic B").await;
    let c3 = seed_org(&c.db.pool, "Clinic C").await;
    let (_, viewer) = c.token(Some(a), Role::Viewer).await;
    let (status, body) = c.get("/v1/orgs", Some(&viewer)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["id"], a.to_string());
    assert_eq!(body["items"][0]["name"], "Clinic A");
    assert!(body["items"][0].get("settings").is_none());
    assert!(body["next_cursor"].is_null());

    let (_, root) = c.token(None, Role::Superadmin).await;
    let (status, p1) = c.get("/v1/orgs?limit=2", Some(&root)).await;
    assert_eq!(status, StatusCode::OK, "{p1}");
    assert_eq!(p1["items"].as_array().unwrap().len(), 2);
    let cursor = p1["next_cursor"].as_str().expect("more pages");
    let (_, p2) = c
        .get(&format!("/v1/orgs?limit=2&cursor={cursor}"), Some(&root))
        .await;
    let mut ids: Vec<String> = p1["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(p2["items"].as_array().unwrap())
        .map(|o| o["id"].as_str().unwrap().to_owned())
        .collect();
    for want in [a, b, c3] {
        assert!(ids.contains(&want.to_string()), "{want}");
    }
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "no duplicates across pages");

    let (status, _) = c.get("/v1/orgs", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    c.db.cleanup().await;
}

#[tokio::test]
async fn org_detail_returns_settings_without_secret_looking_keys() {
    let Some(c) = setup().await else { return };
    let org = seed_org(&c.db.pool, "Clinic A").await;
    sqlx::query("UPDATE organizations SET settings = $2 WHERE id = $1")
        .bind(org)
        .bind(json!({
            "timezone": "Asia/Kathmandu",
            "clinical": {"cosign_policy": "required", "api_key": "leak"},
            "webhook_secret": "leak"
        }))
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (_, admin) = c.token(Some(org), Role::Admin).await;
    let (status, body) = c.get(&format!("/v1/orgs/{org}"), Some(&admin)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["timezone"], "Asia/Kathmandu");
    assert_eq!(body["settings"]["clinical"]["cosign_policy"], "required");
    assert!(!body.to_string().contains("leak"), "{body}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn users_me_reports_user_id_and_totp_enrolment_without_the_secret() {
    let Some(c) = setup().await else { return };
    let org = seed_org(&c.db.pool, "Clinic A").await;
    let (id, tok) = c.token(Some(org), Role::Clinician).await;
    let (status, me) = c.get("/v1/users/me", Some(&tok)).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["user_id"], id.to_string());
    assert_eq!(me["totp_enrolled"], false);

    sqlx::query("UPDATE users SET totp_secret = 'JBSWY3DPEHPK3PXP' WHERE id = $1")
        .bind(id)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (_, me) = c.get("/v1/users/me", Some(&tok)).await;
    assert_eq!(me["totp_enrolled"], true);
    assert!(!me.to_string().contains("JBSWY3DPEHPK3PXP"));
    c.db.cleanup().await;
}

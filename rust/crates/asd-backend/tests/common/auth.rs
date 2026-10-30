//! owner: WP-B.2
//!
//! Auth helpers for integration tests: seed orgs and users, mint access tokens, enrol
//! devices, build JSON requests with a bearer. Used by `auth.rs`, `rbac_matrix.rs`,
//! `audit.rs`, `devices.rs`, and available to every other test binary (B.3 console reads
//! need a user JWT, ingest needs a device token).
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::OnceLock;

use asd_backend::app::AppState;
use asd_backend::auth::{device_token, password};
use asd_contract::api_v1::Role;
use axum::body::Body;
use axum::extract::ConnectInfo;
use http::{Method, Request};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// The password every seeded user has.
pub const PASSWORD: &str = "correct horse battery staple";

/// One Argon2id hash of [`PASSWORD`], computed once per test binary (hashing is the slow
/// part of seeding; the salt being shared between test users does not matter).
fn password_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| password::hash(PASSWORD).expect("hash"))
}

pub async fn seed_org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organizations (name, kind) VALUES ($1, 'clinic') RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("insert org")
}

/// Inserts an active user with [`PASSWORD`]. `org = None` only for `superadmin`.
pub async fn seed_user(pool: &PgPool, org: Option<Uuid>, role: Role, email: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (org_id, email, pw_hash, role) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(org)
    .bind(email)
    .bind(password_hash())
    .bind(role_str(role))
    .fetch_one(pool)
    .await
    .expect("insert user")
}

pub fn role_str(role: Role) -> &'static str {
    asd_backend::auth::rbac::role_as_str(role)
}

/// A valid access token for a seeded user, minted with the state's JWT key (no login, no
/// Argon2, no rate limit).
pub fn access_token(state: &AppState, user_id: Uuid, org: Option<Uuid>, role: Role) -> String {
    let keys = state
        .config
        .auth
        .jwt
        .as_ref()
        .expect("test config has a JWT key");
    keys.issue(user_id, org, role).expect("issue").0
}

/// Inserts an enrol code for `org` and returns the plain code.
pub async fn seed_enrol_code(pool: &PgPool, org: Uuid, max_uses: i32) -> String {
    let code = format!("TEST-{}", &Uuid::new_v4().simple().to_string()[..8]).to_uppercase();
    let hash = asd_backend::auth::enrol_code_hash(&code);
    sqlx::query("INSERT INTO enrol_codes (org_id, code_hash, max_uses) VALUES ($1, $2, $3)")
        .bind(org)
        .bind(hash.as_slice())
        .bind(max_uses)
        .execute(pool)
        .await
        .expect("insert enrol code");
    code
}

/// Inserts a device for `org` directly and returns `(device_id, device_token)`.
pub async fn seed_device(pool: &PgPool, org: Uuid) -> (Uuid, String) {
    let (token, hash) = device_token::new_token();
    let id = sqlx::query_scalar(
        "INSERT INTO devices (org_id, platform, app_version, token_hash) \
         VALUES ($1, 'iOS', '1.0', $2) RETURNING id",
    )
    .bind(org)
    .bind(hash.as_slice())
    .fetch_one(pool)
    .await
    .expect("insert device");
    (id, token)
}

/// A JSON request with an optional bearer.
pub fn json(method: Method, uri: &str, body: &Value, bearer: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header(http::header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        b = b.header(http::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    b.body(Body::from(serde_json::to_vec(body).unwrap()))
        .expect("request builds")
}

/// A body-less request with an optional bearer.
pub fn empty(method: Method, uri: &str, bearer: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        b = b.header(http::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    b.body(Body::empty()).expect("request builds")
}

/// Marks `request` as coming from `ip` (what `into_make_service_with_connect_info` does).
pub fn from_ip(mut request: Request<Body>, ip: &str) -> Request<Body> {
    let addr: SocketAddr = format!("{ip}:40000").parse().expect("ip");
    request.extensions_mut().insert(ConnectInfo(addr));
    request
}

/// Rows in `audit_log` with this action.
pub async fn audit_count(pool: &PgPool, action: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = $1")
        .bind(action)
        .fetch_one(pool)
        .await
        .expect("count audit rows")
}

pub async fn audit_total(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(pool)
        .await
        .expect("count audit rows")
}

/// The problem slug of an error body.
pub fn slug(body: &Value) -> &str {
    body["type"]
        .as_str()
        .and_then(|t| t.rsplit('/').next())
        .unwrap_or("")
}

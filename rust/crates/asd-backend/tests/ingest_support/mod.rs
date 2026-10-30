//! owner: WP-B.3
//!
//! Helpers shared by the WP-B.3 integration tests (`sessions`, `blobs`, `console_read`,
//! `review`): the iOS golden payload, row seeding and request builders.
//!
//! Every request goes through the real WP-B.2 auth: devices are enrolled with an enrol code
//! and `POST /v1/devices/register` (their `dvc_` token is the bearer), users are seeded
//! with the shared test password and log in with `POST /v1/auth/login` (their access token
//! is the bearer). Helpers that take a `device` or `user` string take that bearer token.
#![allow(dead_code)] // each test binary uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use asd_backend::app::AppState;
use asd_backend::auth::rbac;
use asd_backend::blob::{BlobStore, MemoryBlobStore};
use asd_backend::config::Config;
use axum::body::Body;
use axum::Router;
use http::{HeaderMap, Method, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

pub const PROBLEM_BASE: &str = "https://schemas.curv.org/asd/problems/";

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// `apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json` (the iOS oracle).
pub fn golden() -> Value {
    let path = repo_root().join("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("iOS fixture")).expect("json")
}

/// The golden payload with a different `sessionId` (top level and `rawPayload`).
pub fn golden_with_id(id: &str) -> Value {
    let mut v = golden();
    v["sessionId"] = json!(id);
    v["rawPayload"]["sessionId"] = json!(id);
    v
}

/// The full application router (real auth, no test layer).
pub fn app(state: AppState) -> Router {
    common::router(state)
}

pub fn config(allow_synthetic: bool) -> Config {
    let mut c = common::test_config(common::UNREACHABLE_DB_URL);
    c.allow_synthetic = allow_synthetic;
    c
}

pub fn state(pool: PgPool) -> AppState {
    AppState::new(pool, Arc::new(MemoryBlobStore::new()), config(false))
}

pub fn state_with(pool: PgPool, blobs: Arc<dyn BlobStore>, config: Config) -> AppState {
    AppState::new(pool, blobs, config)
}

// ---- seeding ---------------------------------------------------------------------------

pub async fn seed_org(pool: &PgPool, settings: Value) -> Uuid {
    sqlx::query_scalar("INSERT INTO organizations (name, kind, settings) VALUES ('Org', 'clinic', $1) RETURNING id")
        .bind(settings)
        .fetch_one(pool)
        .await
        .expect("seed org")
}

/// An org whose random audit never fires, so tests see only the deterministic rules.
pub async fn seed_quiet_org(pool: &PgPool) -> Uuid {
    seed_org(pool, json!({"audit_rate": 0.0})).await
}

/// Enrols a device of `org` the way the app does: an enrol code, then
/// `POST /v1/devices/register`. Returns `(device_id, device_token)`.
pub async fn seed_device(pool: &PgPool, org: Uuid) -> (Uuid, String) {
    let code = common::auth::seed_enrol_code(pool, org, 1).await;
    // A fresh router per call: the register limiter is per router.
    let (status, _, body) = common::send(
        common::router(state(pool.clone())),
        common::auth::json(
            Method::POST,
            "/v1/devices/register",
            &json!({"enrol_code": code, "platform": "iOS", "app_version": "1.0"}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "register: {body}");
    let id = body["device_id"]
        .as_str()
        .expect("device_id")
        .parse()
        .expect("uuid");
    let token = body["device_token"]
        .as_str()
        .expect("device_token")
        .to_owned();
    (id, token)
}

/// Seeds an active user of `org` with `role` (`org = None` only for `superadmin`) and
/// logs them in. Returns `(user_id, access_token)`.
pub async fn seed_user(pool: &PgPool, org: Option<Uuid>, role: &str) -> (Uuid, String) {
    let role = rbac::role_from_str(role).expect("known role");
    let email = format!("{}@example.org", Uuid::new_v4().simple());
    let id = common::auth::seed_user(pool, org, role, &email).await;
    // A fresh router per call: the login limiter is per router.
    let (status, _, body) = common::send(
        common::router(state(pool.clone())),
        common::auth::json(
            Method::POST,
            "/v1/auth/login",
            &json!({"email": email, "password": common::auth::PASSWORD}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login: {body}");
    assert_eq!(body["user"]["id"], id.to_string());
    let token = body["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned();
    (id, token)
}

/// `Authorization: Bearer <token>`.
pub fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

// ---- requests --------------------------------------------------------------------------

/// `POST /v1/sessions` the way the iOS client sends it (CON-7 headers).
pub fn post_session(device: Option<&str>, body: &Value) -> http::Request<Body> {
    post_session_raw(
        device,
        body.get("sessionId").and_then(Value::as_str),
        Some("1.6"),
        serde_json::to_vec(body).unwrap(),
    )
}

pub fn post_session_raw(
    device: Option<&str>,
    idempotency_key: Option<&str>,
    schema_version: Option<&str>,
    body: Vec<u8>,
) -> http::Request<Body> {
    let mut b = http::Request::builder()
        .method("POST")
        .uri("/v1/sessions")
        .header("content-type", "application/json")
        .header("accept", "application/json");
    if let Some(d) = device {
        b = b.header(http::header::AUTHORIZATION, bearer(d));
    }
    if let Some(k) = idempotency_key {
        b = b.header("x-idempotency-key", k);
    }
    if let Some(v) = schema_version {
        b = b.header("x-schema-version", v);
    }
    b.body(Body::from(body)).unwrap()
}

pub fn user_get(user: &str, uri: &str) -> http::Request<Body> {
    http::Request::builder()
        .uri(uri)
        .header(http::header::AUTHORIZATION, bearer(user))
        .body(Body::empty())
        .unwrap()
}

pub fn user_json(user: &str, method: &str, uri: &str, body: &Value) -> http::Request<Body> {
    http::Request::builder()
        .method(method)
        .uri(uri)
        .header(http::header::AUTHORIZATION, bearer(user))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

/// Sends a request and returns status, headers and raw body bytes.
pub async fn send_raw(
    router: Router,
    request: http::Request<Body>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = router.oneshot(request).await.expect("infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

pub fn problem_slug(body: &Value) -> &str {
    body["type"]
        .as_str()
        .and_then(|t| t.strip_prefix(PROBLEM_BASE))
        .unwrap_or("<not a problem>")
}

/// Posts `body` with the `device` bearer; panics unless 201.
pub async fn ingest_ok(router: &Router, device: &str, body: &Value) -> Value {
    let (status, _, resp) = common::send(router.clone(), post_session(Some(device), body)).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    resp
}

//! owner: WP-C.10
//!
//! Shared fixtures for the worklist, clinical review and signing tests: one org with every
//! role, a second org, sessions with a stored clip, and request helpers for the clinical
//! review endpoints (`If-Match`, step-up sign bodies).
#![allow(dead_code)] // each test binary uses a different subset

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use asd_backend::app::AppState;
use asd_backend::blob::MemoryBlobStore;
use asd_backend::config::Config;
use asd_contract::api_v1::Role;
use axum::body::Body;
use axum::Router;
use http::{HeaderMap, Method, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::auth::{access_token, seed_user, PASSWORD};
use crate::common::{self, send, TestDb};

pub struct User {
    pub id: Uuid,
    pub token: String,
}

pub struct Ctx {
    pub db: TestDb,
    pub state: AppState,
    pub app: Router,
    pub org: Uuid,
    pub other_org: Uuid,
    /// Clinician with a signer profile.
    pub clin_a: User,
    /// Second clinician with a signer profile (cosigner).
    pub clin_b: User,
    pub owner: User,
    pub admin: User,
    pub researcher: User,
    pub reviewer: User,
    pub viewer: User,
    pub superadmin: User,
    /// Clinician of the other org.
    pub other_clin: User,
}

impl Ctx {
    pub fn pool(&self) -> &PgPool {
        &self.db.pool
    }

    pub async fn cleanup(self) {
        self.db.cleanup().await;
    }
}

/// Config like `common::test_config`, with extra variables.
pub fn config_with(database_url: &str, extra: &[(&str, &str)]) -> Config {
    let extra: HashMap<String, String> = extra
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    Config::from_lookup(|key| {
        if let Some(v) = extra.get(key) {
            return (!v.is_empty()).then(|| v.clone());
        }
        match key {
            "DATABASE_URL" => Some(database_url.to_owned()),
            "ASD_BLOB_BACKEND" => Some("memory".to_owned()),
            "ASD_DB_ACQUIRE_TIMEOUT_MS" => Some("500".to_owned()),
            "ASD_JWT_SIGNING_KEY" => Some(common::test_jwt_key().to_owned()),
            "ASD_DATA_KEK" => Some(common::test_data_kek().to_owned()),
            _ => None,
        }
    })
    .expect("test config is valid")
}

/// A state on `pool` with extra config variables (e.g. `ASD_ALLOW_SYNTHETIC`).
pub fn state_with(pool: PgPool, extra: &[(&str, &str)]) -> AppState {
    let config = config_with(common::UNREACHABLE_DB_URL, extra);
    AppState::new(pool, Arc::new(MemoryBlobStore::new()), config)
}

pub async fn seed_org_kind(pool: &PgPool, name: &str, kind: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organizations (name, kind) VALUES ($1, $2) RETURNING id")
        .bind(name)
        .bind(kind)
        .fetch_one(pool)
        .await
        .expect("insert org")
}

async fn user(state: &AppState, pool: &PgPool, org: Option<Uuid>, role: Role, email: &str) -> User {
    let id = seed_user(pool, org, role, email).await;
    User {
        id,
        token: access_token(state, id, org, role),
    }
}

pub async fn set_profile(pool: &PgPool, user: Uuid, name: Option<&str>, reg: Option<&str>) {
    sqlx::query("UPDATE users SET display_name = $2, professional_registration = $3 WHERE id = $1")
        .bind(user)
        .bind(name)
        .bind(reg)
        .execute(pool)
        .await
        .expect("set profile");
}

/// Merges `settings` into the org's settings (top-level keys replaced).
pub async fn set_settings(pool: &PgPool, org: Uuid, settings: Value) {
    sqlx::query("UPDATE organizations SET settings = settings || $2 WHERE id = $1")
        .bind(org)
        .bind(settings)
        .execute(pool)
        .await
        .expect("set settings");
}

pub async fn set_cosign_policy(pool: &PgPool, org: Uuid, policy: &str) {
    set_settings(pool, org, json!({"clinical": {"cosign_policy": policy}})).await;
}

/// Everything a test needs; `None` (skip) without `DATABASE_URL`.
pub async fn setup() -> Option<Ctx> {
    setup_with(&[]).await
}

pub async fn setup_with(extra: &[(&str, &str)]) -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let pool = db.pool.clone();
    let state = state_with(pool.clone(), extra);
    let app = common::router(state.clone());
    let org = seed_org_kind(&pool, "Clinic", "clinic").await;
    let other_org = seed_org_kind(&pool, "Other clinic", "clinic").await;
    let s = &state;
    let clin_a = user(s, &pool, Some(org), Role::Clinician, "a@clinic.test").await;
    let clin_b = user(s, &pool, Some(org), Role::Clinician, "b@clinic.test").await;
    set_profile(&pool, clin_a.id, Some("Dr A Sharma"), Some("NMC 12345")).await;
    set_profile(&pool, clin_b.id, Some("Dr B Thapa"), None).await;
    let other_clin = user(s, &pool, Some(other_org), Role::Clinician, "c@other.test").await;
    set_profile(&pool, other_clin.id, Some("Dr C"), None).await;
    Some(Ctx {
        owner: user(s, &pool, Some(org), Role::Owner, "owner@clinic.test").await,
        admin: user(s, &pool, Some(org), Role::Admin, "admin@clinic.test").await,
        researcher: user(s, &pool, Some(org), Role::Researcher, "res@clinic.test").await,
        reviewer: user(s, &pool, Some(org), Role::Reviewer, "rev@clinic.test").await,
        viewer: user(s, &pool, Some(org), Role::Viewer, "view@clinic.test").await,
        superadmin: user(s, &pool, None, Role::Superadmin, "root@vendor.test").await,
        clin_a,
        clin_b,
        other_clin,
        db,
        state,
        app,
        org,
        other_org,
    })
}

/// One stored metric row (`details.rows[]`, the asd-core `PerVideoScoreRow` shape).
pub fn clip_rows(observed: f64) -> Value {
    json!({"rows": [{
        "segmentId": "seg-1",
        "segmentDescription": "faces vs objects",
        "biomarker": "eye_dwell_ratio",
        "metricKey": "eye_dwell_ratio",
        "direction": "higherBetter",
        "observed": observed,
        "expected": 0.5,
        "acceptableLow": 0.3,
        "acceptableHigh": 0.7,
        "status": "monitor",
        "score": 0.71,
        "imputed": false,
        "citation": "test",
        "source": "norm"
    }]})
}

/// A participant and session `id` in `org` with `flag` and `source`, received
/// `hours_ago` hours ago, with two stored clips (one with metric rows).
pub async fn session_full(
    pool: &PgPool,
    org: Uuid,
    id: &str,
    flag: &str,
    source: &str,
    hours_ago: i64,
) {
    let token = format!("child-{id}");
    let participant: Uuid =
        sqlx::query_scalar("INSERT INTO participants (org_id, token) VALUES ($1, $2) RETURNING id")
            .bind(org)
            .bind(&token)
            .fetch_one(pool)
            .await
            .expect("insert participant");
    sqlx::query(
        "INSERT INTO sessions (id, org_id, participant_id, participant_token, capture_source, \
           started_at, duration_sec, schema_version, quality_verdict, valid_frame_ratio, \
           social_preference, eye_dwell, attention_flag, summary, bundle_version, \
           composite_score, received_at) \
         VALUES ($1, $2, $3, $4, $5, \
           TIMESTAMPTZ '2026-10-01T10:00:00.250Z' - make_interval(hours => $7::int), 31.5, \
           '1.6', 'optimal', 0.93, 0.61, 0.42, $6, '{}', 'b-2026.10', 0.37, \
           now() - make_interval(hours => $7::int))",
    )
    .bind(id)
    .bind(org)
    .bind(participant)
    .bind(&token)
    .bind(source)
    .bind(flag)
    .bind(i32::try_from(hours_ago).unwrap())
    .execute(pool)
    .await
    .expect("insert session");
    sqlx::query(
        "INSERT INTO session_clip_scores (session_id, clip_id, clip_order, dwell_eye, status, \
           score, weight, details) VALUES \
         ($1, 'clip-b', 2, 0.4, 'monitor', 0.7, 1.0, $2), \
         ($1, 'clip-a', 1, NULL, NULL, NULL, NULL, NULL)",
    )
    .bind(id)
    .bind(clip_rows(0.42))
    .execute(pool)
    .await
    .expect("insert clips");
}

pub async fn session(pool: &PgPool, org: Uuid, id: &str, flag: &str) {
    session_full(pool, org, id, flag, "truedepth", 1).await;
}

/// A request with an optional bearer, JSON body and extra headers.
pub fn request(
    method: Method,
    uri: &str,
    body: Option<&Value>,
    bearer: Option<&str>,
    headers: &[(&str, &str)],
) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        b = b.header(http::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    match body {
        Some(v) => b
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(v).unwrap()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

pub type Resp = (StatusCode, HeaderMap, Value);

pub async fn call(ctx: &Ctx, req: Request<Body>) -> Resp {
    send(ctx.app.clone(), req).await
}

pub async fn get_as(ctx: &Ctx, token: &str, uri: &str) -> Resp {
    call(ctx, request(Method::GET, uri, None, Some(token), &[])).await
}

/// `POST /v1/sessions/{id}/clinical-review`.
pub async fn claim(ctx: &Ctx, token: &str, session: &str) -> Resp {
    call(
        ctx,
        request(
            Method::POST,
            &format!("/v1/sessions/{session}/clinical-review"),
            Some(&json!({})),
            Some(token),
            &[],
        ),
    )
    .await
}

/// Claims and asserts success; returns the review.
pub async fn claimed(ctx: &Ctx, token: &str, session: &str) -> Value {
    let (status, _, body) = claim(ctx, token, session).await;
    assert!(status.is_success(), "claim: {status} {body}");
    body
}

/// `PATCH /v1/clinical-reviews/{id}` with `If-Match: "<row_version>"`.
pub async fn patch(
    ctx: &Ctx,
    token: &str,
    review: &Value,
    if_match: Option<&str>,
    body: Value,
) -> Resp {
    let id = review["id"].as_str().unwrap();
    let etag = review["row_version"].to_string();
    let quoted = format!("\"{etag}\"");
    let header = if_match.unwrap_or(&quoted);
    call(
        ctx,
        request(
            Method::PATCH,
            &format!("/v1/clinical-reviews/{id}"),
            Some(&body),
            Some(token),
            &[("if-match", header)],
        ),
    )
    .await
}

/// Patches and asserts success; returns the updated review.
pub async fn patched(ctx: &Ctx, token: &str, review: &Value, body: Value) -> Value {
    let (status, _, out) = patch(ctx, token, review, None, body).await;
    assert_eq!(status, StatusCode::OK, "patch: {out}");
    out
}

pub fn sign_body(hash: &str, password: &str, totp: Option<&str>) -> Value {
    let mut b = json!({"content_hash": hash, "password": password, "meaning_ack": true});
    if let Some(code) = totp {
        b["totp_code"] = json!(code);
    }
    b
}

/// `POST /v1/clinical-reviews/{id}/{action}` (`sign` or `cosign`) with `body`.
pub async fn post_action(
    ctx: &Ctx,
    token: &str,
    review_id: &str,
    action: &str,
    body: &Value,
) -> Resp {
    call(
        ctx,
        request(
            Method::POST,
            &format!("/v1/clinical-reviews/{review_id}/{action}"),
            Some(body),
            Some(token),
            &[],
        ),
    )
    .await
}

/// Signs `review` (its current hash) with the seeded password.
pub async fn sign(ctx: &Ctx, token: &str, review: &Value) -> Resp {
    let body = sign_body(review["content_hash"].as_str().unwrap(), PASSWORD, None);
    post_action(ctx, token, review["id"].as_str().unwrap(), "sign", &body).await
}

pub async fn cosign(ctx: &Ctx, token: &str, review: &Value) -> Resp {
    let body = sign_body(review["content_hash"].as_str().unwrap(), PASSWORD, None);
    post_action(ctx, token, review["id"].as_str().unwrap(), "cosign", &body).await
}

pub async fn amend(ctx: &Ctx, token: &str, review: &Value, reason: &str) -> Resp {
    post_action(
        ctx,
        token,
        review["id"].as_str().unwrap(),
        "amend",
        &json!({"reason": reason}),
    )
    .await
}

/// A claimed review of a new `flag` session with a saved draft, ready to sign.
pub async fn draft_ready(ctx: &Ctx, session_id: &str, flag: &str) -> Value {
    session(ctx.pool(), ctx.org, session_id, flag).await;
    let review = claimed(ctx, &ctx.clin_a.token, session_id).await;
    patched(
        ctx,
        &ctx.clin_a.token,
        &review,
        json!({"disposition": "monitor", "impression": "Reduced eye dwell in clip 2.",
               "recommendations": "Recapture in 3 months."}),
    )
    .await
}

pub async fn audit_rows(
    pool: &PgPool,
    action: &str,
    target: &str,
) -> Vec<(Option<Value>, Option<Value>)> {
    sqlx::query_as(
        "SELECT before, after FROM audit_log WHERE action = $1 AND target_id = $2 ORDER BY id",
    )
    .bind(action)
    .bind(target)
    .fetch_all(pool)
    .await
    .expect("audit rows")
}

pub async fn signature_count(pool: &PgPool, review_id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM clinical_signatures WHERE review_id = $1::uuid")
        .bind(review_id)
        .fetch_one(pool)
        .await
        .expect("count signatures")
}

// ---- on_review_finalized test double (SIGN-10) ---------------------------------------------

fn finalized() -> &'static Mutex<HashMap<Uuid, u32>> {
    static CALLS: OnceLock<Mutex<HashMap<Uuid, u32>>> = OnceLock::new();
    CALLS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn record_finalized(review_id: Uuid) {
    *finalized().lock().unwrap().entry(review_id).or_insert(0) += 1;
}

/// Installs the counting observer (once per test binary).
pub fn observe_finalized() {
    asd_backend::interop::set_finalize_observer(record_finalized);
}

/// How often `interop::on_review_finalized` ran for `review_id` (it runs just before commit).
pub fn finalized_calls(review_id: &str) -> u32 {
    let id = Uuid::parse_str(review_id).unwrap();
    finalized().lock().unwrap().get(&id).copied().unwrap_or(0)
}

//! owner: WP-B.11
//!
//! AUTH-6..8 and AUTH-10 (ADR-0009, acceptance 17): self-service TOTP enrolment under
//! `/v1/users/me/totp`, through the real router. Needs `DATABASE_URL` (skipped otherwise)
//! except the tests marked "no database".

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use asd_backend::app::AppState;
use asd_backend::auth::totp;
use asd_backend::auth::ManualClock;
use asd_backend::clinical::reauth::{self, totp_code, StepUpLimiter};
use asd_backend::telemetry::RedactingJsonLayer;
use asd_contract::api_v1::Role;
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use uuid::Uuid;

const EMAIL: &str = "Clinician+totp@Example.org";
const ENROL: &str = "/v1/users/me/totp/enrol";
const CONFIRM: &str = "/v1/users/me/totp/confirm";
const TOTP: &str = "/v1/users/me/totp";

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
    user: Uuid,
    bearer: String,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let mut state = state_with_pool(db.pool.clone());
    // Every request of a test falls into one rate-limit window.
    state.clock = ManualClock::new();
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let user = seed_user(&db.pool, Some(org), Role::Clinician, EMAIL).await;
    let bearer = access_token(&state, user, Some(org), Role::Clinician);
    Some(Ctx {
        db,
        state,
        app,
        user,
        bearer,
    })
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn code_for(secret: &str) -> String {
    totp_code(secret, now_unix()).unwrap()
}

/// A well-formed code that is not the current one (nor, but for a tiny chance, a
/// neighbour's).
fn wrong_code_for(secret: &str) -> String {
    format!(
        "{:06}",
        (code_for(secret).parse::<u32>().unwrap() + 500_000) % 1_000_000
    )
}

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Value,
    bearer: &str,
) -> (StatusCode, Value) {
    let (status, _, body) = send(app.clone(), json(method, uri, &body, Some(bearer))).await;
    (status, body)
}

async fn enrol(c: &Ctx) -> (StatusCode, Value) {
    call(
        &c.app,
        Method::POST,
        ENROL,
        json!({"password": PASSWORD}),
        &c.bearer,
    )
    .await
}

async fn confirm(c: &Ctx, code: &str) -> (StatusCode, Value) {
    call(
        &c.app,
        Method::POST,
        CONFIRM,
        json!({"totp_code": code}),
        &c.bearer,
    )
    .await
}

async fn login(app: &Router, code: Option<&str>) -> (StatusCode, Value) {
    let mut body = json!({"email": EMAIL, "password": PASSWORD});
    if let Some(code) = code {
        body["totp_code"] = json!(code);
    }
    let (status, _, body) = send(
        app.clone(),
        from_ip(
            json(Method::POST, "/v1/auth/login", &body, None),
            "198.51.100.7",
        ),
    )
    .await;
    (status, body)
}

async fn me(c: &Ctx) -> Value {
    let (status, _, body) = send(
        c.app.clone(),
        empty(Method::GET, "/v1/users/me", Some(&c.bearer)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Enrols and confirms; returns the seed.
async fn enrol_and_confirm(c: &Ctx) -> String {
    let (status, body) = enrol(c).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let secret = body["secret"].as_str().unwrap().to_owned();
    let (status, body) = confirm(c, &code_for(&secret)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    secret
}

// ---- AUTH-6, AUTH-7: enrol and confirm ---------------------------------------------------

#[tokio::test]
async fn enrol_then_confirm_makes_login_need_a_code() {
    let Some(c) = setup().await else { return };

    let (status, body) = enrol(&c).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let secret = body["secret"].as_str().unwrap().to_owned();
    // 20 random bytes in RFC 4648 base32 without padding.
    assert_eq!(secret.len(), 32, "{secret}");
    assert!(secret
        .bytes()
        .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b)));
    let url = body["otpauth_url"].as_str().unwrap();
    assert!(
        url.starts_with("otpauth://totp/Eye%20Gaze%20Console:"),
        "{url}"
    );
    assert!(url.contains(&format!("secret={secret}")), "{url}");
    assert!(chrono::DateTime::parse_from_rfc3339(body["expires_at"].as_str().unwrap()).is_ok());

    // Pending only: login still needs no code (AUTH-10).
    let (status, body) = login(&c.app, None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "pending seed must not affect login: {body}"
    );

    let (status, body) = confirm(&c, &code_for(&secret)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"totp_enrolled": true}));
    assert_eq!(audit_count(&c.db.pool, "user.totp_enrolled").await, 1);

    // Booleans, so no decrypted value or ciphertext is needed here.
    let row: (bool, bool, bool) = sqlx::query_as(
        "SELECT totp_secret_ct IS NOT NULL AND totp_secret_kek_id IS NOT NULL,
                last_totp_step IS NOT NULL,
                totp_pending_ct IS NULL AND totp_pending_kek_id IS NULL
                    AND totp_pending_expires_at IS NULL
         FROM users WHERE id = $1",
    )
    .bind(c.user)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert!(row.0, "seed moved to totp_secret_ct");
    assert!(row.1, "the confirming step is spent");
    assert!(row.2, "pending cleared");

    // The confirming code is spent: login with the same code fails.
    let (status, body) = login(&c.app, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "totp-required");

    let me = me(&c).await;
    assert_eq!(me["totp_enrolled"], true);
    assert_eq!(me["totp_pending"], false);
    c.db.cleanup().await;
}

#[tokio::test]
async fn login_with_a_fresh_code_after_enrolment_succeeds() {
    let Some(c) = setup().await else { return };
    let secret = enrol_and_confirm(&c).await;
    // The confirming step is spent; the next step's code is accepted (one step of skew).
    let next = totp_code(&secret, now_unix() + 30).unwrap();
    let (status, body) = login(&c.app, Some(&next)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn wrong_code_is_401_and_audited() {
    let Some(c) = setup().await else { return };
    let (_, body) = enrol(&c).await;
    let secret = body["secret"].as_str().unwrap().to_owned();
    let (status, body) = confirm(&c, &wrong_code_for(&secret)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "totp-invalid");
    assert_eq!(audit_count(&c.db.pool, "auth.totp_enrol_failed").await, 1);
    // Still pending, still usable.
    assert_eq!(me(&c).await["totp_pending"], true);
    let (status, _) = confirm(&c, &code_for(&secret)).await;
    assert_eq!(status, StatusCode::OK);
    c.db.cleanup().await;
}

#[tokio::test]
async fn expired_or_absent_pending_seed_is_410() {
    let Some(c) = setup().await else { return };
    let (status, body) = confirm(&c, "123456").await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert_eq!(slug(&body), "totp-enrolment-expired");

    let (_, body) = enrol(&c).await;
    let secret = body["secret"].as_str().unwrap().to_owned();
    sqlx::query(
        "UPDATE users SET totp_pending_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(c.user)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (status, body) = confirm(&c, &code_for(&secret)).await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert_eq!(slug(&body), "totp-enrolment-expired");
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_second_enrol_replaces_the_pending_seed() {
    let Some(c) = setup().await else { return };
    let (_, first) = enrol(&c).await;
    let (status, second) = enrol(&c).await;
    assert_eq!(status, StatusCode::OK);
    let (a, b) = (
        first["secret"].as_str().unwrap(),
        second["secret"].as_str().unwrap(),
    );
    assert_ne!(a, b);
    let (status, _) = confirm(&c, &code_for(a)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the replaced seed no longer confirms"
    );
    let (status, _) = confirm(&c, &code_for(b)).await;
    assert_eq!(status, StatusCode::OK);
    c.db.cleanup().await;
}

#[tokio::test]
async fn enrol_while_enrolled_is_409() {
    let Some(c) = setup().await else { return };
    enrol_and_confirm(&c).await;
    let (status, body) = enrol(&c).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(slug(&body), "totp-already-enrolled");
    c.db.cleanup().await;
}

#[tokio::test]
async fn enrol_with_a_wrong_password_is_401_and_audited() {
    let Some(c) = setup().await else { return };
    let (status, body) = call(
        &c.app,
        Method::POST,
        ENROL,
        json!({"password": "not the password"}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "invalid-credentials");
    assert_eq!(audit_count(&c.db.pool, "auth.password_failed").await, 1);
    assert_eq!(me(&c).await["totp_pending"], false);
    c.db.cleanup().await;
}

#[tokio::test]
async fn enrol_without_the_kek_is_503() {
    let Some(mut c) = setup().await else { return };
    c.state.keyring = None;
    let app = router(c.state.clone());
    let (status, body) = call(
        &app,
        Method::POST,
        ENROL,
        json!({"password": PASSWORD}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(slug(&body), "clinical-crypto-unavailable");
    c.db.cleanup().await;
}

#[tokio::test]
async fn sixth_enrol_in_a_minute_is_429() {
    let Some(c) = setup().await else { return };
    for i in 0..5 {
        let (status, body) = enrol(&c).await;
        assert_eq!(status, StatusCode::OK, "attempt {i}: {body}");
    }
    let (status, body) = enrol(&c).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn unauthenticated_calls_are_401() {
    let Some(c) = setup().await else { return };
    let (status, _, _) = send(
        c.app.clone(),
        json(Method::POST, ENROL, &json!({"password": PASSWORD}), None),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    c.db.cleanup().await;
}

// ---- AUTH-8: disable --------------------------------------------------------------------

#[tokio::test]
async fn disable_needs_password_and_code() {
    let Some(c) = setup().await else { return };
    let secret = enrol_and_confirm(&c).await;

    let (status, body) = call(
        &c.app,
        Method::DELETE,
        TOTP,
        json!({"password": PASSWORD}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "totp-required");

    let next = totp_code(&secret, now_unix() + 30).unwrap();
    let (status, body) = call(
        &c.app,
        Method::DELETE,
        TOTP,
        json!({"password": "wrong password", "totp_code": next}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "invalid-credentials");

    let (status, body) = call(
        &c.app,
        Method::DELETE,
        TOTP,
        json!({"password": PASSWORD, "totp_code": next}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"totp_enrolled": false}));
    assert_eq!(audit_count(&c.db.pool, "user.totp_disabled").await, 1);
    let step: Option<i64> = sqlx::query_scalar("SELECT last_totp_step FROM users WHERE id = $1")
        .bind(c.user)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(step, None);

    // Login needs no code again.
    let (status, body) = login(&c.app, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(me(&c).await["totp_enrolled"], false);
    c.db.cleanup().await;
}

#[tokio::test]
async fn disable_with_a_spent_code_is_401() {
    let Some(c) = setup().await else { return };
    let secret = enrol_and_confirm(&c).await;
    // The confirm spent the current step.
    let (status, body) = call(
        &c.app,
        Method::DELETE,
        TOTP,
        json!({"password": PASSWORD, "totp_code": code_for(&secret)}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "totp-invalid");
    assert_eq!(me(&c).await["totp_enrolled"], true);
    c.db.cleanup().await;
}

#[tokio::test]
async fn disable_when_not_enrolled_is_404() {
    let Some(c) = setup().await else { return };
    let (status, body) = call(
        &c.app,
        Method::DELETE,
        TOTP,
        json!({"password": PASSWORD, "totp_code": "123456"}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(slug(&body), "totp-not-enrolled");
    c.db.cleanup().await;
}

// ---- races, repeats and the shared limiter ---------------------------------------------

#[tokio::test]
async fn two_concurrent_confirms_enrol_exactly_once() {
    let Some(c) = setup().await else { return };
    let (_, body) = enrol(&c).await;
    let code = code_for(body["secret"].as_str().unwrap());
    let (a, b) = tokio::join!(confirm(&c, &code), confirm(&c, &code));
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::CONFLICT],
        "{} / {}",
        a.1,
        b.1
    );
    let loser = if a.0 == StatusCode::CONFLICT {
        &a.1
    } else {
        &b.1
    };
    assert_eq!(slug(loser), "totp-already-enrolled");
    assert_eq!(audit_count(&c.db.pool, "user.totp_enrolled").await, 1);
    assert_eq!(me(&c).await["totp_enrolled"], true);
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_second_confirm_after_success_is_409_already_enrolled() {
    let Some(c) = setup().await else { return };
    let (_, body) = enrol(&c).await;
    let code = code_for(body["secret"].as_str().unwrap());
    let (status, _) = confirm(&c, &code).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = confirm(&c, &code).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(slug(&body), "totp-already-enrolled");
    assert_eq!(audit_count(&c.db.pool, "user.totp_enrolled").await, 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn an_expired_confirm_clears_the_pending_seed() {
    let Some(c) = setup().await else { return };
    let (_, body) = enrol(&c).await;
    let secret = body["secret"].as_str().unwrap().to_owned();
    sqlx::query(
        "UPDATE users SET totp_pending_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(c.user)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (status, _) = confirm(&c, &code_for(&secret)).await;
    assert_eq!(status, StatusCode::GONE);
    let cleared: bool = sqlx::query_scalar(
        "SELECT totp_pending_ct IS NULL AND totp_pending_kek_id IS NULL
                AND totp_pending_expires_at IS NULL
         FROM users WHERE id = $1",
    )
    .bind(c.user)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert!(cleared);
    c.db.cleanup().await;
}

#[tokio::test]
async fn enrol_disable_login_without_code_then_enrol_again() {
    let Some(c) = setup().await else { return };
    let first = enrol_and_confirm(&c).await;
    let next = totp_code(&first, now_unix() + 30).unwrap();
    let (status, body) = call(
        &c.app,
        Method::DELETE,
        TOTP,
        json!({"password": PASSWORD, "totp_code": next}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = login(&c.app, None).await;
    assert_eq!(status, StatusCode::OK, "no code after disable: {body}");

    let second = enrol_and_confirm(&c).await;
    assert_ne!(first, second, "a new seed");
    let (status, body) = login(&c.app, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(slug(&body), "totp-required");
    let (status, body) = login(&c.app, Some(&code_for(&first))).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the old seed is gone: {body}"
    );
    let next = totp_code(&second, now_unix() + 30).unwrap();
    let (status, body) = login(&c.app, Some(&next)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(audit_count(&c.db.pool, "user.totp_enrolled").await, 2);
    c.db.cleanup().await;
}

/// Enrol, confirm and disable draw on one 5-per-minute budget per user.
#[tokio::test]
async fn confirm_and_disable_attempts_share_the_enrol_limiter() {
    let Some(c) = setup().await else { return };
    let (_, body) = enrol(&c).await; // 1
    let secret = body["secret"].as_str().unwrap().to_owned();
    for _ in 0..2 {
        let (status, _) = confirm(&c, &wrong_code_for(&secret)).await; // 2, 3
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    for _ in 0..2 {
        let (status, _) = call(
            &c.app,
            Method::DELETE,
            TOTP,
            json!({"password": PASSWORD, "totp_code": "123456"}),
            &c.bearer,
        )
        .await; // 4, 5
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, body) = confirm(&c, &code_for(&secret)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    let (status, body) = call(
        &c.app,
        Method::DELETE,
        TOTP,
        json!({"password": PASSWORD, "totp_code": "123456"}),
        &c.bearer,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(
        me(&c).await["totp_enrolled"],
        false,
        "the limited confirm did nothing"
    );
    c.db.cleanup().await;
}

// ---- AUTH-9 helper for B.14 -------------------------------------------------------------

#[tokio::test]
async fn reset_clears_every_totp_column_and_audits() {
    let Some(c) = setup().await else { return };
    enrol_and_confirm(&c).await;
    // A database where the 0016 back-fill has not run still holds the seed in plaintext
    // (one form only: `users_totp_secret_one_form`).
    sqlx::query(
        "UPDATE users SET totp_secret = 'JBSWY3DPEHPK3PXP', totp_secret_ct = NULL,
                          totp_secret_kek_id = NULL
         WHERE id = $1",
    )
    .bind(c.user)
    .execute(&c.db.pool)
    .await
    .unwrap();
    assert!(totp::reset(&c.db.pool, c.user).await.unwrap());
    assert!(!totp::reset(&c.db.pool, Uuid::new_v4()).await.unwrap());
    let plaintext: Option<String> =
        sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = $1")
            .bind(c.user)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(
        plaintext, None,
        "reset clears the legacy plaintext seed too"
    );
    let me = me(&c).await;
    assert_eq!(me["totp_enrolled"], false);
    assert_eq!(me["totp_pending"], false);
    assert_eq!(audit_count(&c.db.pool, "user.totp_reset").await, 1);
    let (status, body) = login(&c.app, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    c.db.cleanup().await;
}

// ---- AUTH-10: a pending seed is never consulted -----------------------------------------

#[tokio::test]
async fn pending_seed_is_ignored_by_login_and_step_up() {
    let Some(c) = setup().await else { return };
    let (_, body) = enrol(&c).await;
    let secret = body["secret"].as_str().unwrap().to_owned();
    assert!(
        totp::load_secret(&c.db.pool, c.state.keyring.as_deref(), c.user)
            .await
            .unwrap()
            .is_none()
    );
    let (status, body) = login(&c.app, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Step-up with the password only passes: the user is not enrolled yet.
    reauth::step_up(&c.state, &StepUpLimiter::new(), c.user, PASSWORD, None)
        .await
        .expect("pending seed must not be required by step-up");
    // A code from the pending seed is not what makes step-up pass either way.
    reauth::step_up(
        &c.state,
        &StepUpLimiter::new(),
        c.user,
        PASSWORD,
        Some(&code_for(&secret)),
    )
    .await
    .expect("stray code ignored");
    let me = me(&c).await;
    assert_eq!(me["totp_enrolled"], false);
    assert_eq!(me["totp_pending"], true);
    c.db.cleanup().await;
}

#[tokio::test]
async fn users_me_reports_totp_pending() {
    let Some(c) = setup().await else { return };
    assert_eq!(me(&c).await["totp_pending"], false);
    enrol(&c).await;
    assert_eq!(me(&c).await["totp_pending"], true);
    // An expired pending seed is not "pending".
    sqlx::query(
        "UPDATE users SET totp_pending_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(c.user)
    .execute(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(me(&c).await["totp_pending"], false);
    c.db.cleanup().await;
}

#[tokio::test]
async fn pending_columns_are_checked_together() {
    let Some(c) = setup().await else { return };
    let err = sqlx::query("UPDATE users SET totp_pending_ct = '\\x00' WHERE id = $1")
        .bind(c.user)
        .execute(&c.db.pool)
        .await
        .expect_err("ciphertext without kek id is refused");
    assert!(err.to_string().contains("users_totp_pending"), "{err}");
    c.db.cleanup().await;
}

// ---- otpauth URL (no database) ----------------------------------------------------------

#[test]
fn otpauth_url_percent_encodes_the_label_and_issuer() {
    let url = totp::otpauth_url("a+b@example.org", "JBSWY3DPEHPK3PXP");
    assert_eq!(
        url,
        "otpauth://totp/Eye%20Gaze%20Console:a%2Bb%40example.org?secret=JBSWY3DPEHPK3PXP\
         &issuer=Eye%20Gaze%20Console&algorithm=SHA1&digits=6&period=30"
    );
    assert_eq!(totp::TOTP_ISSUER, "Eye Gaze Console");
    assert_eq!(
        totp::pending_aad(Uuid::nil()).to_string(),
        "users:totp_pending_ct:00000000-0000-0000-0000-000000000000:00000000-0000-0000-0000-000000000000"
    );
}

#[test]
fn new_secrets_are_random_base32_of_20_bytes() {
    let a = totp::new_secret();
    let b = totp::new_secret();
    assert_ne!(*a, *b);
    assert_eq!(a.len(), 32);
    assert!(totp_code(&a, 0).is_some(), "valid base32");
}

// ---- OBS-1: neither the seed nor a code reaches the log --------------------------------

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(self.0.clone())
    }
}

#[test]
fn logs_contain_neither_the_seed_nor_the_code() {
    let cap = Capture::default();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen_in = seen.clone();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("trace"))
        .with(RedactingJsonLayer::new(cap.clone()));
    tracing::subscriber::with_default(subscriber, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let Some(c) = setup().await else { return };
                let (_, body) = enrol(&c).await;
                let secret = body["secret"].as_str().unwrap().to_owned();
                let wrong = wrong_code_for(&secret);
                confirm(&c, &wrong).await;
                let code = code_for(&secret);
                let (status, _) = confirm(&c, &code).await;
                assert_eq!(status, StatusCode::OK);
                let next = totp_code(&secret, now_unix() + 30).unwrap();
                let (status, _) = call(
                    &c.app,
                    Method::DELETE,
                    TOTP,
                    json!({"password": PASSWORD, "totp_code": next}),
                    &c.bearer,
                )
                .await;
                assert_eq!(status, StatusCode::OK);
                seen_in.lock().unwrap().extend([secret, wrong, code, next]);
                c.db.cleanup().await;
            });
    });
    let needles = seen.lock().unwrap().clone();
    if needles.is_empty() {
        return; // skipped: no database
    }
    let text = String::from_utf8(cap.0.lock().unwrap().clone()).unwrap();
    assert!(
        !text.is_empty(),
        "nothing was logged; the test would be vacuous"
    );
    let (seed, codes) = (&needles[0], &needles[1..]);
    for needle in [seed.as_str(), PASSWORD] {
        assert!(!text.contains(needle), "`{needle}` leaked into the log");
    }
    // Six-digit codes: compare whole digit runs, so a timestamp's digits do not match.
    let runs: Vec<&str> = text
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|r| !r.is_empty())
        .collect();
    for code in codes {
        assert!(
            !runs.contains(&code.as_str()),
            "code `{code}` leaked into the log"
        );
    }
}

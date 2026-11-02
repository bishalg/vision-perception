//! owner: WP-B.11
//!
//! OBS-1 for the TOTP routes (AUTH-6..8): neither the seed, the password nor a code reaches
//! the log. Needs `DATABASE_URL` (skipped otherwise).
//!
//! A binary of its own, with this one test: `tracing::subscriber::with_default` is
//! thread-scoped but callsite interest is global and cached. With a single registered
//! dispatcher, tracing-core computes a new callsite's interest from the *registering*
//! thread's default, so a callsite that a parallel test thread (no subscriber) hits first
//! is cached as "never" and this test's events vanish ("nothing was logged"). Alone in its
//! binary, no other thread hits a callsite while the subscriber is installed.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use asd_backend::auth::ManualClock;
use asd_backend::clinical::reauth::totp_code;
use asd_backend::telemetry::RedactingJsonLayer;
use asd_contract::api_v1::Role;
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;

const EMAIL: &str = "Clinician+totp-logs@Example.org";
const ENROL: &str = "/v1/users/me/totp/enrol";
const CONFIRM: &str = "/v1/users/me/totp/confirm";
const TOTP: &str = "/v1/users/me/totp";

struct Ctx {
    db: TestDb,
    app: Router,
    bearer: String,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let mut state = state_with_pool(db.pool.clone());
    state.clock = ManualClock::new();
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let user = seed_user(&db.pool, Some(org), Role::Clinician, EMAIL).await;
    let bearer = access_token(&state, user, Some(org), Role::Clinician);
    Some(Ctx { db, app, bearer })
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
        // Callsites hit before the subscriber was registered keep a cached interest.
        tracing::callsite::rebuild_interest_cache();
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

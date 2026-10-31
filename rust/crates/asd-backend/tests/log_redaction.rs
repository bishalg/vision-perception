//! owner: WP-B.1
//!
//! OBS-1: logs are JSON lines; fields named `participant_token`, `email`, `notes`,
//! `rawPayload` (and secrets) are dropped, e-mail addresses and participant tokens in any
//! value are masked, and no participant token from a request path reaches a log line.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use asd_backend::telemetry::RedactingJsonLayer;
use common::*;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;

const TOKEN: &str = "child-SESS-GOL";
const ENROLLED_TOKEN: &str = "P-4471-enrolled";
const EMAIL: &str = "clinician@example.org";

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

impl Capture {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
    fn lines(&self) -> Vec<serde_json::Value> {
        self.text()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON ({e}): {l}")))
            .collect()
    }
}

/// Runs `f` (inside a current-thread runtime) with the redacting layer installed.
fn capture<F, Fut>(f: F) -> Capture
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let cap = Capture::default();
    let subscriber = tracing_subscriber::registry().with(RedactingJsonLayer::new(cap.clone()));
    tracing::subscriber::with_default(subscriber, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f());
    });
    cap
}

fn assert_clean(cap: &Capture) {
    let text = cap.text();
    assert!(
        !text.is_empty(),
        "nothing was logged; the test would be vacuous"
    );
    for needle in [TOKEN, ENROLLED_TOKEN, EMAIL] {
        assert!(
            !text.contains(needle),
            "`{needle}` leaked into the log:\n{text}"
        );
    }
}

#[test]
fn every_line_is_json_with_standard_keys() {
    let cap = capture(|| async {
        tracing::info!(session_count = 3, "ingest batch");
    });
    let lines = cap.lines();
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line["level"], "INFO");
    assert_eq!(line["message"], "ingest batch");
    assert_eq!(line["fields"]["session_count"], 3);
    assert!(line["target"].as_str().unwrap().contains("log_redaction"));
    assert!(chrono::DateTime::parse_from_rfc3339(line["timestamp"].as_str().unwrap()).is_ok());
}

#[test]
fn sensitive_named_fields_are_dropped() {
    let cap = capture(|| async {
        tracing::info!(
            participant_token = TOKEN,
            participantToken = ENROLLED_TOKEN,
            email = EMAIL,
            notes = "child cried during clip 3",
            rawPayload = ?serde_json::json!({"participantId": ENROLLED_TOKEN}),
            raw_payload = "x",
            password = "hunter2",
            authorization = "Bearer dvc_secret",
            device_token = "dvc_secret",
            refresh_token = "rt_secret",
            session_id = "SESS-GOLDEN-0001",
            "session received"
        );
    });
    assert_clean(&cap);
    let line = &cap.lines()[0];
    let fields = line["fields"].as_object().unwrap();
    for dropped in [
        "participant_token",
        "participantToken",
        "email",
        "notes",
        "rawPayload",
        "raw_payload",
        "password",
        "authorization",
        "device_token",
        "refresh_token",
    ] {
        assert!(
            !fields.contains_key(dropped),
            "field `{dropped}` was not dropped: {line}"
        );
    }
    let text = cap.text();
    for secret in ["hunter2", "dvc_secret", "rt_secret", "child cried"] {
        assert!(!text.contains(secret), "`{secret}` leaked: {text}");
    }
    assert_eq!(fields["session_id"], "SESS-GOLDEN-0001");
}

#[test]
fn emails_and_child_tokens_in_free_text_are_masked() {
    let cap = capture(|| async {
        tracing::warn!(detail = %format!("user {EMAIL} uploaded for {TOKEN}"), "upload for {} by {}", TOKEN, EMAIL);
    });
    assert_clean(&cap);
    let line = &cap.lines()[0];
    let message = line["message"].as_str().unwrap();
    assert!(message.contains("[redacted-email]"), "{message}");
    assert!(message.contains("[redacted-token]"), "{message}");
}

#[test]
fn span_fields_are_redacted_too() {
    let cap = capture(|| async {
        let span = tracing::info_span!(
            "ingest",
            participant_token = TOKEN,
            org = "org-1",
            who = EMAIL
        );
        let _guard = span.enter();
        tracing::info!("inside span");
    });
    assert_clean(&cap);
    let line = &cap.lines()[0];
    let span = &line["spans"][0];
    assert_eq!(span["name"], "ingest");
    assert_eq!(span["org"], "org-1");
    assert!(span.get("participant_token").is_none(), "{line}");
}

#[test]
fn http_request_logs_never_contain_the_path_token() {
    let cap = capture(|| async {
        let app = router(state_without_db());
        let (status, _, _) = send(
            app.clone(),
            get(&format!("/v1/participants/{TOKEN}/summary?email={EMAIL}")),
        )
        .await;
        // WP-B.6 made the summary a real route: no bearer is 401 and the log carries the
        // route template, never the token.
        assert_eq!(status, http::StatusCode::UNAUTHORIZED);
        let (status, _, _) = send(
            app.clone(),
            get(&format!("/v1/participants/{TOKEN}/unknown?email={EMAIL}")),
        )
        .await;
        assert_eq!(status, http::StatusCode::NOT_FOUND);
        let (status, _, _) = send(app, get(&format!("/health?p={ENROLLED_TOKEN}"))).await;
        assert_eq!(status, http::StatusCode::OK);
    });
    assert_clean(&cap);
    let text = cap.text();
    assert!(
        text.contains("\"route\":\"/health\""),
        "matched route is logged: {text}"
    );
    assert!(
        text.contains("\"route\":\"<unmatched>\""),
        "unmatched path is hidden: {text}"
    );
    assert!(
        text.contains("\"status\":404"),
        "response status is logged: {text}"
    );
}

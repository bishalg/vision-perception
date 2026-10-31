//! owner: WP-C.11
//!
//! NOTE-1: note text and MRNs never reach the logs. Kept in its own test binary: a scoped
//! tracing subscriber misses callsites that concurrently running tests (with no subscriber)
//! cached as "never interested", which made this check flaky inside `notes.rs`.
//! Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod participant_support;

use std::io::Write;
use std::sync::{Arc, Mutex};

use asd_backend::telemetry::RedactingJsonLayer;
use asd_contract::api_v1::Role;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::json;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;

const SECRET: &str = "Parent reports rocking and no pointing, see Dr Rai";

fn notes_url(p: impl std::fmt::Display) -> String {
    format!("/v1/participants/{p}/notes")
}

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

static RAN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[test]
fn note_text_and_mrn_never_reach_the_logs() {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let subscriber =
        tracing_subscriber::registry().with(RedactingJsonLayer::new(Capture(buf.clone())));
    tracing::subscriber::with_default(subscriber, || {
        // Other tests in this binary run concurrently with no subscriber; a callsite they hit
        // first is cached as "never interested" and would skip this scoped subscriber. Rebuild
        // the cache now that the capture subscriber exists (fixes a flaky "nothing was logged").
        tracing::callsite::rebuild_interest_cache();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let Some(c) = Ctx::new().await else { return };
                RAN.store(true, std::sync::atomic::Ordering::SeqCst);
                c.set_mode(c.org, "mrn").await;
                let p = c.participant(c.org, "P-1").await;
                let (_, clin) = c.token(Role::Clinician).await;
                let (s, _) = c
                    .call(
                        Method::POST,
                        &notes_url(p),
                        Some(json!({"body": SECRET})),
                        &clin,
                    )
                    .await;
                assert_eq!(s, StatusCode::CREATED);
                let (s, _) = c.call(Method::GET, &notes_url(p), None, &clin).await;
                assert_eq!(s, StatusCode::OK);
                let (s, _) = c
                    .call(
                        Method::PUT,
                        &format!("/v1/participants/{p}/patient-ref"),
                        Some(json!({"assigning_authority": "TUTH", "mrn": "TU-777-LOGCHECK"})),
                        &clin,
                    )
                    .await;
                assert_eq!(s, StatusCode::CREATED);
                // A failing request logs too.
                let (s, _) = c
                    .call(
                        Method::POST,
                        &notes_url(p),
                        Some(json!({"body": ""})),
                        &clin,
                    )
                    .await;
                assert_eq!(s, StatusCode::BAD_REQUEST);
                c.db.cleanup().await;
            });
    });
    let text = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    if RAN.load(std::sync::atomic::Ordering::SeqCst) {
        assert!(
            !text.is_empty(),
            "nothing was logged; the test would be vacuous"
        );
    }
    for needle in ["rocking", "no pointing", "LOGCHECK", "TU-777"] {
        assert!(!text.contains(needle), "log holds {needle:?}: {text}");
    }
}

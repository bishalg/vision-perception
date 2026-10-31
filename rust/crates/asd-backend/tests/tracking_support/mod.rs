//! owner: WP-G.11
//!
//! Schema 1.7 helpers shared by `sessions`, `console_read` and `review`: the iOS 1.7 golden
//! payload (`expected-session-v1.7.json`, 1.6 plus `rawPayload.trackingQuality`) and a
//! `POST /v1/sessions` request with `X-Schema-Version: 1.7`.
#![allow(dead_code)] // each test binary uses a different subset

use axum::body::Body;
use axum::Router;
use http::StatusCode;
use serde_json::{json, Value};

use crate::common;
use crate::ingest_support::{post_session_raw, repo_root};

/// `apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json`.
pub fn golden_v1_7() -> Value {
    let path = repo_root().join("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("iOS 1.7 fixture")).expect("json")
}

/// The 1.7 golden with a different `sessionId` (top level and `rawPayload`).
pub fn golden_v1_7_with_id(id: &str) -> Value {
    let mut v = golden_v1_7();
    v["sessionId"] = json!(id);
    v["rawPayload"]["sessionId"] = json!(id);
    v
}

/// The 1.7 golden with tracking verdict `verdict` and estimated accuracy `accuracy`.
pub fn golden_v1_7_tracking(id: &str, verdict: &str, accuracy: Option<f64>) -> Value {
    let mut v = golden_v1_7_with_id(id);
    let tq = &mut v["rawPayload"]["trackingQuality"];
    tq["verdict"] = json!(verdict);
    match accuracy {
        Some(a) => tq["estimatedAccuracyDeg"] = json!(a),
        None => {
            tq.as_object_mut().unwrap().remove("estimatedAccuracyDeg");
        }
    }
    v
}

/// `POST /v1/sessions` with `X-Schema-Version: 1.7`.
pub fn post_session_v1_7(device: Option<&str>, body: &Value) -> http::Request<Body> {
    post_session_raw(
        device,
        body.get("sessionId").and_then(Value::as_str),
        Some("1.7"),
        serde_json::to_vec(body).unwrap(),
    )
}

/// Posts a 1.7 `body` with the `device` bearer; panics unless 201.
pub async fn ingest_v1_7_ok(router: &Router, device: &str, body: &Value) -> Value {
    let (status, _, resp) =
        common::send(router.clone(), post_session_v1_7(Some(device), body)).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    resp
}

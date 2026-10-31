//! owner: WP-I.7
//!
//! Signed webhook delivery (DLV-1, DLV-2, DLV-5, INTG-4) against a local receiver. This
//! binary relaxes the network guard for loopback (`allow_loopback_for_tests`); everything
//! else about the guard is covered by `tests/netguard.rs`. Needs `DATABASE_URL`.

mod common;
mod delivery_support;
mod export_support;
mod participant_support;

use asd_backend::interop::{netguard, webhook};
use asd_contract::api_v1::Role;
use base64::Engine as _;
use delivery_support::{verify_standard_webhook, Receiver, PUBLIC_BASE};
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};

fn relax() {
    netguard::allow_loopback_for_tests();
}

/// The published Standard Webhooks test vector.
#[test]
fn signature_matches_the_standard_webhooks_reference_vector() {
    let secret = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";
    let key = base64::engine::general_purpose::STANDARD
        .decode(secret.trim_start_matches("whsec_"))
        .unwrap();
    assert_eq!(
        webhook::signature(
            &key,
            "msg_p5jXN8AQM9LWM0D4loKWxJek",
            1_614_265_330,
            br#"{"test": 2432232314}"#
        ),
        "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE="
    );
}

/// An org with one consented participant session and a production webhook destination;
/// returns (clinician token, destination id, secret).
async fn setup(c: &Ctx, rx: &Receiver) -> (String, String, String) {
    let (_, owner) = c.token(Role::Owner).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let p = c.participant(c.org, "TOKEN-ALPHA").await;
    c.exportable_session(
        p,
        "TOKEN-ALPHA",
        "SESS-ONE",
        "2026-10-01T10:00:00Z",
        Some(true),
    )
    .await;
    let (id, secret) = c
        .webhook_destination(&owner, &rx.url("/hook"), json!({}))
        .await;
    (clin, id, secret)
}

async fn export_to(c: &Ctx, clin: &str, destination: &str) -> String {
    let mut body = export_support::body("csv", "identified", "clinical_care");
    body["destination_ids"] = json!([destination]);
    let (s, made) = c.request_export(clin, body).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    made["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn delivers_a_signed_event_with_a_download_link_and_no_clinical_data() {
    relax();
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let rx = Receiver::start().await;
    let (clin, dest, secret) = setup(&c, &rx).await;
    let export_id = export_to(&c, &clin, &dest).await;
    c.run_jobs().await;

    let hits = rx.hits();
    assert_eq!(hits.len(), 1, "one POST");
    let hit = &hits[0];
    assert_eq!(hit.path, "/hook");
    assert_eq!(hit.header("content-type"), "application/json");
    assert!(verify_standard_webhook(&secret, hit), "signature verifies");
    let ts: i64 = hit.header("webhook-timestamp").parse().unwrap();
    assert!((chrono::Utc::now().timestamp() - ts).abs() < 60);

    let event = hit.json();
    let mut keys: Vec<&str> = event
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "created_at",
            "download",
            "export_id",
            "format",
            "id",
            "mode",
            "org_id",
            "type"
        ]
    );
    assert_eq!(event["type"], "export.ready");
    assert_eq!(event["export_id"], export_id);
    assert_eq!(event["id"], hit.header("webhook-id"));
    let body = String::from_utf8(hit.body.clone()).unwrap();
    for leak in [
        "TOKEN-ALPHA",
        "SESS-ONE",
        "eye_dwell",
        "mrn",
        "impression",
        "participant",
    ] {
        assert!(!body.contains(leak), "webhook body leaks {leak}: {body}");
    }

    // The link is absolute, short-lived and downloads the artifact through the proxy.
    let url = event["download"]["url"].as_str().unwrap();
    assert!(url.starts_with(&format!("{PUBLIC_BASE}/v1/exports/{export_id}/file?t=")));
    let expires: chrono::DateTime<chrono::Utc> = event["download"]["expires_at"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let left = (expires - chrono::Utc::now()).num_seconds();
    assert!((800..=900).contains(&left), "15 minutes, got {left}s");
    let path = url.trim_start_matches(PUBLIC_BASE);
    let response = tower::ServiceExt::oneshot(c.app.clone(), common::get(path))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(bytes.to_vec(), c.artifact(&export_id).await);

    let rows = c.deliveries_of(&export_id).await;
    assert_eq!(
        rows,
        [("delivered".into(), 1, None, Some("http 200".into()))]
    );
    let audit = c.audit_rows("export.deliver").await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["after"]["remote_ref"], "http 200");
    assert!(!c.audit_dump().await.contains("/hook?"));

    // Re-running finds nothing to do (idempotent).
    c.make_jobs_due().await;
    c.run_jobs().await;
    assert_eq!(rx.hits().len(), 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn failures_back_off_1m_5m_30m_2h_12h_then_fail_with_an_error_class_only() {
    relax();
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let rx = Receiver::start().await;
    rx.respond(503, "REMOTE-SECRET-BODY maintenance");
    let (clin, dest, _) = setup(&c, &rx).await;
    let export_id = export_to(&c, &clin, &dest).await;
    c.run_jobs().await;

    for (attempt, expected) in [
        (1, 60.0),
        (2, 300.0),
        (3, 1_800.0),
        (4, 7_200.0),
        (5, 43_200.0),
    ] {
        let rows = c.deliveries_of(&export_id).await;
        assert_eq!(
            rows,
            [(
                "pending".into(),
                attempt,
                Some("http-status".into()),
                Some("http 503".into())
            )],
            "after attempt {attempt}"
        );
        let delay = c.delivery_job_delay(&export_id).await;
        assert!(
            (delay - expected).abs() < 10.0,
            "attempt {attempt}: run_after in {delay}s"
        );
        c.make_jobs_due().await;
        c.run_jobs().await;
    }
    let rows = c.deliveries_of(&export_id).await;
    assert_eq!(rows[0].0, "failed");
    assert_eq!(rows[0].1, 6);
    assert_eq!(rx.hits().len(), 6);
    let (status, error): (String, String) = sqlx::query_as(
        "SELECT j.status, j.error FROM jobs j JOIN export_deliveries d ON d.job_id = j.id
         WHERE d.export_id = $1::uuid",
    )
    .bind(&export_id)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(error, "http-status (http 503)");
    // The remote body is stored nowhere.
    let everywhere: String = sqlx::query_scalar(
        "SELECT coalesce(string_agg(t::text, ' '), '') FROM (
            SELECT to_jsonb(j)::text AS t FROM jobs j
            UNION ALL SELECT to_jsonb(d)::text FROM export_deliveries d
            UNION ALL SELECT to_jsonb(a)::text FROM audit_log a) x",
    )
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert!(!everywhere.contains("REMOTE-SECRET-BODY"));
    assert_eq!(c.audit_rows("export.deliver_failed").await.len(), 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn redirects_are_not_followed() {
    relax();
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let rx = Receiver::start().await;
    rx.respond(302, "");
    let (clin, dest, _) = setup(&c, &rx).await;
    let export_id = export_to(&c, &clin, &dest).await;
    c.run_jobs().await;
    let hits = rx.hits();
    assert_eq!(hits.len(), 1);
    assert!(hits.iter().all(|h| h.path != "/redirected"));
    // A redirect needs someone to fix the URL: no retry.
    let rows = c.deliveries_of(&export_id).await;
    assert_eq!(
        rows,
        [(
            "failed".into(),
            1,
            Some("http-redirect".into()),
            Some("http 302".into())
        )]
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn missing_secret_fails_without_sending() {
    relax();
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let rx = Receiver::start().await;
    let (clin, _, _) = setup(&c, &rx).await;
    let (_, owner) = c.token(Role::Owner).await;
    let bare = c
        .destination(
            &owner,
            json!({"kind": "webhook", "name": "No secret", "config": {"url": rx.url("/bare")}}),
        )
        .await;
    let export_id = export_to(&c, &clin, bare["id"].as_str().unwrap()).await;
    c.run_jobs().await;
    assert!(rx.hits().is_empty());
    assert_eq!(
        c.deliveries_of(&export_id).await[0],
        ("failed".into(), 1, Some("missing-secret".into()), None)
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn connection_test_sends_a_synthetic_record_and_stores_no_remote_body() {
    relax();
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let rx = Receiver::start().await;
    let (_, owner) = c.token(Role::Owner).await;
    let (id, secret) = c
        .webhook_destination(&owner, &rx.url("/hook"), json!({}))
        .await;
    let uri = format!("/v1/integrations/{id}/test");

    let (s, r) = c.call(Method::POST, &uri, None, &owner).await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert_eq!(r["ok"], true);
    assert_eq!(r["remote_ref"], "http 200");
    let hit = &rx.hits()[0];
    assert!(verify_standard_webhook(&secret, hit));
    let event = hit.json();
    assert_eq!(event["test"], true);
    assert_eq!(event["test_record"]["participant_token"], "TEST-0000");
    assert_eq!(event["download"], Value::Null);
    let (_, d) = c
        .call(Method::GET, &format!("/v1/integrations/{id}"), None, &owner)
        .await;
    assert_eq!(d["last_test_ok"], true);
    assert!(d["last_test_at"].is_string());

    rx.respond(500, "REMOTE-STACKTRACE secret=abc");
    let (s, r) = c.call(Method::POST, &uri, None, &owner).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["ok"], false);
    assert_eq!(r["error_class"], "http-status");
    assert!(!r.to_string().contains("REMOTE-STACKTRACE"));
    let (_, d) = c
        .call(Method::GET, &format!("/v1/integrations/{id}"), None, &owner)
        .await;
    assert_eq!(d["last_test_ok"], false);
    assert_eq!(d["last_test_error"], "http-status");
    assert!(!c.audit_dump().await.contains("REMOTE-STACKTRACE"));
    assert_eq!(c.audit_rows("integration.test").await.len(), 2);
    c.db.cleanup().await;
}

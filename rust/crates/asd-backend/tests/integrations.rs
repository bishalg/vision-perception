//! owner: WP-I.7
//!
//! Integration settings (INTG-1..5): per-kind config rules, write-only secrets (GET shows
//! a fingerprint only; audit rows hold fingerprints only), SSH key generation, RBAC and
//! org scoping, audit copies without URL queries, and the delivery rows an export request
//! creates. Production network policy (no loopback). Needs `DATABASE_URL`.

mod common;
mod delivery_support;
mod export_support;
mod participant_support;

use asd_contract::api_v1::Role;
use http::{Method, StatusCode};
use serde_json::{json, Value};

fn webhook_body(url: &str) -> Value {
    json!({"kind": "webhook", "name": "Hospital HIS", "config": {"url": url}})
}

#[tokio::test]
async fn webhook_secret_is_write_only_and_audited_by_fingerprint() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, owner) = c.token(Role::Owner).await;
    let d = c
        .destination(
            &owner,
            webhook_body("https://his.example.org/hook?token=q-secret"),
        )
        .await;
    assert_eq!(d["kind"], "webhook");
    assert_eq!(d["environment"], "test", "default environment");
    assert_eq!(d["status"], "active");
    assert_eq!(d["has_secret"], false);
    assert!(d["secret_fingerprint"].is_null());
    let id = d["id"].as_str().unwrap();

    let secret = delivery_support::webhook_secret();
    let (s, put) = c
        .call(
            Method::PUT,
            &format!("/v1/integrations/{id}/secret"),
            Some(json!({"secret": secret})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{put}");
    assert_eq!(put["has_secret"], true);
    let fp = put["secret_fingerprint"].as_str().unwrap().to_owned();
    assert_eq!(fp.len(), 8);
    assert!(fp.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert!(!put.to_string().contains(&secret));

    let (s, got) = c
        .call(Method::GET, &format!("/v1/integrations/{id}"), None, &owner)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert!(!got.to_string().contains(&secret));
    assert_eq!(got["secret_fingerprint"], fp);

    // Stored encrypted, never in clear text.
    let ct: Vec<u8> =
        sqlx::query_scalar("SELECT secret_ct FROM integration_destinations WHERE id = $1::uuid")
            .bind(id)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&ct).contains("whsec_"));

    // Audit: fingerprint only; the URL's query string never.
    let dump = c.audit_dump().await;
    assert!(!dump.contains(&secret), "secret leaked into the audit log");
    assert!(
        !dump.contains("q-secret"),
        "url query leaked into the audit log"
    );
    let set = c.audit_rows("integration.secret_set").await;
    assert_eq!(set.len(), 1);
    assert_eq!(set[0]["after"]["secret_fingerprint"], fp);
    assert_eq!(c.audit_rows("integration.create").await.len(), 1);

    // Bad secrets are refused without echoing them.
    let (s, b) = c
        .call(
            Method::PUT,
            &format!("/v1/integrations/{id}/secret"),
            Some(json!({"secret": "plain-password-123"})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(!b.to_string().contains("plain-password-123"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn config_rules_per_kind() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, admin) = c.token(Role::Admin).await;
    let uri = format!("/v1/orgs/{}/integrations", c.org);
    let cases = [
        (webhook_body("http://his.example.org/hook"), "config.url"),
        (webhook_body("https://u:p@his.example.org/"), "config.url"),
        (
            json!({"kind": "webhook", "name": "x", "config": {}}),
            "config.url",
        ),
        (
            json!({"kind": "fhir_server", "name": "x", "config": {}}),
            "kind",
        ),
        (
            json!({"kind": "hl7_mllp", "name": "x", "config": {}}),
            "kind",
        ),
        (
            json!({"kind": "sftp", "name": "x", "config": {
                "host": "sftp.example.org", "username": "curv", "remote_dir": "/in"}}),
            "config.host_key_sha256",
        ),
        (
            json!({"kind": "sftp", "name": "x", "config": {
                "host": "sftp.example.org", "username": "curv", "remote_dir": "in/../x",
                "host_key_sha256": "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU"}}),
            "config.remote_dir",
        ),
        (
            json!({"kind": "webhook", "name": "x", "config": {
                "url": "https://his.example.org/", "trigger": "on_final_sign"}}),
            "config.format",
        ),
        (
            json!({"kind": "webhook", "name": "", "config": {"url": "https://his.example.org/"}}),
            "name",
        ),
    ];
    for (body, field) in cases {
        let (s, b) = c.call(Method::POST, &uri, Some(body.clone()), &admin).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {b}");
        assert!(
            b["invalid_params"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["name"] == field),
            "{body}: expected {field} in {b}"
        );
    }
    // Unknown keys (a smuggled secret) are refused by the schema.
    let (s, _) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"kind": "webhook", "name": "x",
                        "config": {"url": "https://his.example.org/", "password": "p"}})),
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // A valid SFTP destination.
    let d = c
        .destination(
            &admin,
            json!({"kind": "sftp", "name": "HIS drop", "environment": "production", "config": {
                "host": "10.20.0.5", "username": "curv", "remote_dir": "/in",
                "host_key_sha256": "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU",
                "trigger": "on_final_sign", "format": "hl7v2"}}),
        )
        .await;
    assert_eq!(d["config"]["port"], Value::Null);
    assert_eq!(d["environment"], "production");
    c.db.cleanup().await;
}

#[tokio::test]
async fn sftp_key_generation_and_kind_specific_endpoints() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, owner) = c.token(Role::Owner).await;
    let sftp = c
        .destination(
            &owner,
            json!({"kind": "sftp", "name": "HIS drop", "config": {
                "host": "sftp.example.org", "username": "curv", "remote_dir": "/in",
                "host_key_sha256": "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU"}}),
        )
        .await;
    let id = sftp["id"].as_str().unwrap();
    let (s, key) = c
        .call(
            Method::POST,
            &format!("/v1/integrations/{id}/ssh-key"),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{key}");
    let public = key["public_key"].as_str().unwrap();
    assert!(public.starts_with("ssh-ed25519 "), "{public}");
    assert!(key["fingerprint"].as_str().unwrap().starts_with("SHA256:"));
    let (_, got) = c
        .call(Method::GET, &format!("/v1/integrations/{id}"), None, &owner)
        .await;
    assert_eq!(got["ssh_public_key"], public);
    assert_eq!(got["has_secret"], true);
    // Rotation gives a new key.
    let (_, again) = c
        .call(
            Method::POST,
            &format!("/v1/integrations/{id}/ssh-key"),
            None,
            &owner,
        )
        .await;
    assert_ne!(again["public_key"], key["public_key"]);
    let gen = c.audit_rows("integration.ssh_key_generate").await;
    assert_eq!(gen.len(), 2);
    assert!(gen[0]["after"]["ssh_fingerprint"].is_string());

    // SFTP takes no password-like secret; webhooks take no SSH key.
    let (s, _) = c
        .call(
            Method::PUT,
            &format!("/v1/integrations/{id}/secret"),
            Some(json!({"secret": delivery_support::webhook_secret()})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let wh = c
        .destination(&owner, webhook_body("https://his.example.org/hook"))
        .await;
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/integrations/{}/ssh-key", wh["id"].as_str().unwrap()),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    c.db.cleanup().await;
}

#[tokio::test]
async fn rbac_and_org_scope() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, owner) = c.token(Role::Owner).await;
    let d = c
        .destination(&owner, webhook_body("https://his.example.org/hook"))
        .await;
    let id = d["id"].as_str().unwrap();
    let list = format!("/v1/orgs/{}/integrations", c.org);
    for role in [
        Role::Clinician,
        Role::Researcher,
        Role::Reviewer,
        Role::Viewer,
    ] {
        let (_, t) = c.token(role).await;
        let (s, _) = c.call(Method::GET, &list, None, &t).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?} list");
        let (s, _) = c
            .call(
                Method::PATCH,
                &format!("/v1/integrations/{id}"),
                Some(json!({"name": "y"})),
                &t,
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?} patch");
    }
    let (_, admin) = c.token(Role::Admin).await;
    let (s, page) = c.call(Method::GET, &list, None, &admin).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);

    // Another org's admin sees nothing.
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, other_admin) = c.token_in(Some(other), Role::Admin).await;
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/integrations/{id}"),
            None,
            &other_admin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/integrations/{id}/test"),
            None,
            &other_admin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // PATCH: rename, disable; audit before/after without URL queries.
    let (s, patched) = c
        .call(
            Method::PATCH,
            &format!("/v1/integrations/{id}"),
            Some(json!({"status": "disabled", "environment": "production",
                        "config": {"url": "https://his.example.org/v2?k=qq-secret"}})),
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{patched}");
    assert_eq!(patched["status"], "disabled");
    assert_eq!(
        patched["config"]["url"],
        "https://his.example.org/v2?k=qq-secret"
    );
    let rows = c.audit_rows("integration.update").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["after"]["config"]["url"],
        "https://his.example.org/v2?…"
    );
    assert!(!c.audit_dump().await.contains("qq-secret"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn export_requests_create_pending_deliveries() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
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
    let d = c
        .destination(&owner, webhook_body("https://his.example.org/hook"))
        .await;
    let mut body = export_support::body("csv", "identified", "clinical_care");
    body["destination_ids"] = json!([d["id"]]);
    let (s, made) = c.request_export(&clin, body).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    let export_id = made["id"].as_str().unwrap();
    // A delivery row exists with the export; its job is queued only when the export is done.
    let rows: Vec<(String, Option<uuid::Uuid>, String)> = sqlx::query_as(
        "SELECT status, job_id, event FROM export_deliveries WHERE export_id = $1::uuid",
    )
    .bind(export_id)
    .fetch_all(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "pending");
    assert!(rows[0].1.is_none());
    assert_eq!(rows[0].2, "export.ready");

    // The export view lists its deliveries.
    let (s, page) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{export_id}/deliveries"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{page}");
    assert_eq!(page["items"][0]["status"], "pending");
    assert_eq!(page["items"][0]["destination_name"], "Hospital HIS");
    let (s, page) = c
        .call(
            Method::GET,
            &format!("/v1/integrations/{}/deliveries", d["id"].as_str().unwrap()),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(page["total"], 1);
    c.db.cleanup().await;
}

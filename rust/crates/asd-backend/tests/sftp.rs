//! owner: WP-I.7
//!
//! SFTP drop (DLV-4, INTG-5) against an in-process `russh` server: `.part` then rename,
//! the `.sha256` file, the generated key authenticates, a host-key mismatch fails before
//! authentication with nothing uploaded, and the connection test drops `TEST-0000` only.
//! Loopback is allowed in this binary. Needs `DATABASE_URL`.

mod common;
mod delivery_support;
mod export_support;
mod participant_support;

use asd_backend::interop::netguard;
use asd_contract::api_v1::Role;
use delivery_support::SftpServer;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::json;
use sha2::{Digest, Sha256};

/// An SFTP destination for `server` pinned to `fingerprint`, with a generated key the
/// server accepts; returns (owner token, clinician token, destination id).
async fn setup(c: &Ctx, server: &SftpServer, fingerprint: &str) -> (String, String, String) {
    netguard::allow_loopback_for_tests();
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
        .destination(
            &owner,
            json!({"kind": "sftp", "name": "HIS drop", "environment": "production",
                   "config": server.config(fingerprint)}),
        )
        .await;
    let id = d["id"].as_str().unwrap().to_owned();
    let (s, key) = c
        .call(
            Method::POST,
            &format!("/v1/integrations/{id}/ssh-key"),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{key}");
    server.authorize(key["public_key"].as_str().unwrap());
    (owner, clin, id)
}

async fn export_to(c: &Ctx, clin: &str, destination: &str) -> (String, String) {
    let mut body = export_support::body("csv", "identified", "clinical_care");
    body["destination_ids"] = json!([destination]);
    let (s, made) = c.request_export(clin, body).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    let id = made["id"].as_str().unwrap().to_owned();
    c.run_jobs().await;
    let (_, done) = c
        .call(Method::GET, &format!("/v1/exports/{id}"), None, clin)
        .await;
    (
        id,
        done["file_name"].as_str().unwrap_or_default().to_owned(),
    )
}

#[tokio::test]
async fn drops_part_then_renames_with_a_sha256_file() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let server = SftpServer::start().await;
    let (_, clin, dest) = setup(&c, &server, &server.host_key_fingerprint.clone()).await;
    let (export_id, file_name) = export_to(&c, &clin, &dest).await;
    assert!(!file_name.is_empty());

    let artifact = c.artifact(&export_id).await;
    let files = server.files();
    let final_path = format!("/inbound/{file_name}");
    assert_eq!(
        files.get(&final_path),
        Some(&artifact),
        "artifact dropped intact"
    );
    let sha = format!("{:x}", Sha256::digest(&artifact));
    assert_eq!(
        String::from_utf8(files[&format!("{final_path}.sha256")].clone()).unwrap(),
        format!("{sha}  {file_name}\n")
    );
    assert_eq!(files.len(), 2, "no .part left behind: {:?}", files.keys());

    // Each file was written as `.part` and renamed into place; the checksum first.
    let ops = server.ops();
    let part = format!("/inbound/.{file_name}.part");
    let open_part = ops
        .iter()
        .position(|o| *o == format!("open {part}"))
        .unwrap();
    let rename = ops
        .iter()
        .position(|o| *o == format!("rename {part} -> {final_path}"))
        .unwrap();
    assert!(open_part < rename);
    let sha_rename = ops
        .iter()
        .position(|o| o.ends_with(&format!("-> {final_path}.sha256")))
        .unwrap();
    assert!(
        sha_rename < open_part,
        "checksum lands before the artifact: {ops:?}"
    );
    assert!(ops.contains(&"auth curv".to_owned()));

    let rows = c.deliveries_of(&export_id).await;
    assert_eq!(rows[0].0, "delivered");
    assert_eq!(
        rows[0].3.as_deref(),
        Some(final_path.replace("/inbound", "sftp:/inbound").as_str())
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn host_key_mismatch_fails_before_authentication_and_uploads_nothing() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let server = SftpServer::start().await;
    let wrong = asd_backend::interop::ssh_keys::generate()
        .unwrap()
        .fingerprint;
    let (_, clin, dest) = setup(&c, &server, &wrong).await;
    let (export_id, _) = export_to(&c, &clin, &dest).await;

    assert!(server.files().is_empty());
    assert!(
        !server.ops().iter().any(|o| o.starts_with("auth")),
        "no credentials offered to an unpinned server: {:?}",
        server.ops()
    );
    // Never retried into trust-on-first-use.
    assert_eq!(
        c.deliveries_of(&export_id).await,
        [("failed".into(), 1, Some("host-key-mismatch".into()), None)]
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_key_the_server_does_not_know_is_auth_failed() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let server = SftpServer::start().await;
    let (_, clin, dest) = setup(&c, &server, &server.host_key_fingerprint.clone()).await;
    server.authorize(
        &asd_backend::interop::ssh_keys::generate()
            .unwrap()
            .public_openssh,
    );
    let (export_id, _) = export_to(&c, &clin, &dest).await;
    assert!(server.files().is_empty());
    assert_eq!(
        c.deliveries_of(&export_id).await,
        [("failed".into(), 1, Some("auth-failed".into()), None)]
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn connection_test_drops_only_the_synthetic_record() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let server = SftpServer::start().await;
    let (owner, _, dest) = setup(&c, &server, &server.host_key_fingerprint.clone()).await;
    let (s, r) = c
        .call(
            Method::POST,
            &format!("/v1/integrations/{dest}/test"),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert_eq!(r["ok"], true, "{r}");
    let files = server.files();
    let body = String::from_utf8(files["/inbound/curv-test-TEST-0000.json"].clone()).unwrap();
    assert!(body.contains("TEST-0000"));
    assert!(!body.contains("TOKEN-ALPHA"));
    assert!(files.contains_key("/inbound/curv-test-TEST-0000.json.sha256"));
    c.db.cleanup().await;
}

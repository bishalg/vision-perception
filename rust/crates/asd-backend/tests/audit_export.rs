//! owner: WP-B.23
//!
//! `audit_csv` exports (HIP-3, REC-8 `AuditExport`): the request rules, who may ask and
//! download, the filters, formula escaping, the audited download, and a file that holds ids
//! and hashes only (no clinical text, no `before`/`after` payloads). Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use asd_contract::api_v1::Role;
use export_support::body;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const CLINICAL: &str = "CLINICAL-SENTINEL observed poor eye contact";
const PAYLOAD_KEY: &str = "payload-key-sentinel";

fn audit_body(extra: Value) -> Value {
    let mut b = body("audit_csv", "deidentified", "quality_improvement");
    b["reason"] = json!("Quarterly access review of the audit trail");
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        b[k] = v;
    }
    b
}

fn uuid(v: &Value) -> Uuid {
    v.as_str().unwrap().parse().unwrap()
}

async fn put_audit(
    c: &Ctx,
    actor: &str,
    action: &str,
    ts: &str,
    before: Option<Value>,
    after: Option<Value>,
) {
    sqlx::query(
        "INSERT INTO audit_log (ts, actor_type, actor_id, org_id, action, target_type, target_id,
                                before, after, ip, user_agent)
         VALUES ($1::timestamptz, 'user', $2, $3, $4, 'note', 'T-1', $5, $6, '10.1.2.3', 'UA-SENTINEL')",
    )
    .bind(ts)
    .bind(actor)
    .bind(c.org)
    .bind(action)
    .bind(before)
    .bind(after)
    .execute(&c.db.pool)
    .await
    .unwrap();
}

/// Rows of the artifact after the header.
fn lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .skip(1)
        .map(str::to_owned)
        .collect()
}

async fn seed(c: &Ctx) -> (String, String) {
    let alice = Uuid::new_v4().to_string();
    let bob = Uuid::new_v4().to_string();
    put_audit(
        c,
        &alice,
        "note.create",
        "2026-09-01T10:00:00Z",
        None,
        Some(json!({"body": CLINICAL, PAYLOAD_KEY: 1})),
    )
    .await;
    put_audit(
        c,
        &bob,
        "note.update",
        "2026-10-05T10:00:00Z",
        Some(json!({"body": CLINICAL})),
        Some(json!({"body": "other clinical words"})),
    )
    .await;
    put_audit(
        c,
        &alice,
        "export.create",
        "2026-10-20T10:00:00Z",
        None,
        None,
    )
    .await;
    // Another org's row never appears.
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    sqlx::query(
        "INSERT INTO audit_log (actor_type, org_id, action, target_type) VALUES ('system', $1, 'other-org.action', 'x')",
    )
    .bind(other)
    .execute(&c.db.pool)
    .await
    .unwrap();
    (alice, bob)
}

#[tokio::test]
async fn only_owner_and_admin_may_request_an_audit_export() {
    let Some(c) = Ctx::new().await else { return };
    for role in [Role::Owner, Role::Admin] {
        let (_, t) = c.token(role).await;
        let (s, b) = c.request_export(&t, audit_body(json!({}))).await;
        assert_eq!(s, StatusCode::ACCEPTED, "{role:?}: {b}");
        assert_eq!(b["format"], "audit_csv");
    }
    for role in [Role::Researcher, Role::Clinician, Role::Superadmin] {
        let (_, t) = c.token(role).await;
        let (s, b) = c.request_export(&t, audit_body(json!({}))).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?}: {b}");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn mode_purpose_destinations_and_filters_are_pinned() {
    let Some(c) = Ctx::new().await else { return };
    let (_, owner) = c.token(Role::Owner).await;
    let cases = [
        json!({"mode": "identified", "purpose": "clinical_care"}),
        json!({"purpose": "research"}),
        json!({"purpose": "clinical_care"}),
        json!({"mode": "identified"}),
        json!({"filters": {"flag": "red"}}),
        json!({"filters": {"participant_ids": [Uuid::new_v4()]}}),
        json!({"filters": {"actor_id": "not-a-uuid"}}),
        json!({"filters": {"action_prefix": ""}}),
        json!({"filters": {"action_prefix": "x".repeat(65)}}),
        json!({"filters": {"from": "last week"}}),
        json!({"destination_ids": [Uuid::new_v4()]}),
    ];
    for extra in cases {
        let (s, b) = c.request_export(&owner, audit_body(extra.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{extra}: {b}");
        assert_eq!(slug_of(&b), "validation-failed", "{extra}: {b}");
    }
    // The audit-only filters are refused on other formats (step 0) and accepted here.
    let ok = audit_body(json!({"filters": {
        "from": "2026-01-01", "to": "2026-12-31",
        "actor_id": Uuid::new_v4(), "action_prefix": "export."}}));
    let (s, b) = c.request_export(&owner, ok).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_file_holds_the_orgs_rows_as_ids_and_hashes_only() {
    let Some(c) = Ctx::new().await else { return };
    let (alice, _) = seed(&c).await;
    let (_, owner) = c.token(Role::Owner).await;
    let done = c.export_and_run(&owner, audit_body(json!({}))).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["content_type"], "text/csv; charset=utf-8");
    let id = done["id"].as_str().unwrap();
    let bytes = c.artifact(id).await;
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert_eq!(
        text.lines().next().unwrap(),
        "audit_id,ts,actor_type,actor_id,action,target_type,target_id,before_sha256,after_sha256"
    );
    let rows = lines(&bytes);
    // Three seeded rows, the owner's own `export.create`, and any earlier audit rows of
    // this org (user seeding); none of another org.
    assert_eq!(done["row_count"], rows.len() as u64);
    assert!(rows.iter().any(|r| r.contains(",note.create,")));
    assert!(!text.contains("other-org.action"));
    // No clinical text, no payload keys, no network fields, anywhere in the file.
    for banned in [
        CLINICAL,
        "other clinical words",
        PAYLOAD_KEY,
        "UA-SENTINEL",
        "10.1.2.3",
    ] {
        assert!(!text.contains(banned), "{banned} leaked");
    }
    // The hash of an `after` payload is present and is the payload's digest.
    let note = rows.iter().find(|r| r.contains(",note.create,")).unwrap();
    let cols: Vec<&str> = note.split(',').collect();
    assert_eq!(cols[3], alice);
    assert_eq!(cols[7], "", "no before payload");
    assert_eq!(cols[8].len(), 64);
    // No export_items; the sha256 on the row matches the artifact.
    assert!(c.export_items(id).await.is_empty());
    let hex: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(done["sha256"], hex);
    assert_eq!(c.audit_rows("export.complete").await.len(), 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn filters_narrow_the_rows() {
    let Some(c) = Ctx::new().await else { return };
    let (alice, bob) = seed(&c).await;
    let (_, owner) = c.token(Role::Owner).await;
    let run = |f: Value| {
        let c = &c;
        let owner = owner.clone();
        async move {
            let done = c
                .export_and_run(&owner, audit_body(json!({"filters": f})))
                .await;
            assert_eq!(done["status"], "succeeded", "{done}");
            let rows = lines(&c.artifact(done["id"].as_str().unwrap()).await);
            assert_eq!(done["row_count"], rows.len() as u64);
            rows
        }
    };
    let by_actor = run(json!({"actor_id": bob})).await;
    assert_eq!(by_actor.len(), 1);
    assert!(by_actor[0].contains(",note.update,"));
    let by_prefix = run(json!({"action_prefix": "note."})).await;
    assert_eq!(by_prefix.len(), 2);
    let window = run(json!({"from": "2026-10-01", "to": "2026-10-05"})).await;
    assert_eq!(
        window.len(),
        1,
        "to is inclusive to the end of the day: {window:?}"
    );
    assert!(window[0].contains(",note.update,"));
    let combined =
        run(json!({"actor_id": alice, "action_prefix": "note.", "from": "2026-08-01"})).await;
    assert_eq!(combined.len(), 1);
    assert!(combined[0].contains(",note.create,"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn formula_cells_are_escaped() {
    let Some(c) = Ctx::new().await else { return };
    let actor = Uuid::new_v4().to_string();
    put_audit(
        &c,
        &actor,
        "=HYPERLINK(1)",
        "2026-10-01T10:00:00Z",
        None,
        None,
    )
    .await;
    let (_, owner) = c.token(Role::Owner).await;
    let done = c
        .export_and_run(&owner, audit_body(json!({"filters": {"actor_id": actor}})))
        .await;
    let rows = lines(&c.artifact(done["id"].as_str().unwrap()).await);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains(",'=HYPERLINK(1),"), "{}", rows[0]);
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_download_is_audited_and_limited_to_audit_roles() {
    let Some(c) = Ctx::new().await else { return };
    seed(&c).await;
    let (_, owner) = c.token(Role::Owner).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let done = c.export_and_run(&owner, audit_body(json!({}))).await;
    let id = uuid(&done["id"]);
    let url = format!("/v1/exports/{id}/download-url");
    let (s, b) = c.call(Method::POST, &url, None, &researcher).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    assert!(c.audit_rows("export.download").await.is_empty());
    let (s, b) = c.call(Method::POST, &url, None, &owner).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(c.audit_rows("export.download").await.len(), 1);
    c.db.cleanup().await;
}

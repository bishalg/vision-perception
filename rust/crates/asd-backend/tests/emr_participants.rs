//! owner: WP-B.19
//!
//! REC-1 participant create (acceptance 1): `POST /v1/orgs/{id}/participants` mints a
//! `child-<20 hex>` token and answers the QR payload; a device that later posts a session
//! for that token lands on the same participant; the ingest-shaped upsert links to a row a
//! device created first instead of conflicting; RBAC (viewer 403, superadmin allowed, other
//! org 404). Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod ingest_support;
mod participant_support;

use asd_backend::routes::participants::{upsert_clinic_participant, NewParticipant};
use asd_contract::api_v1::Role;
use asd_contract::emr::{is_participant_token, parse_qr_payload};
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn create_url(org: Uuid) -> String {
    format!("/v1/orgs/{org}/participants")
}

#[tokio::test]
async fn admin_creates_a_participant_with_a_minted_token_and_qr() {
    let Some(c) = Ctx::new().await else { return };
    let (_, admin) = c.token(Role::Admin).await;

    let (s, made) = c
        .call(
            Method::POST,
            &create_url(c.org),
            Some(json!({
                "age_bracket_at_enrol": "24-36m",
                "language": "ne",
                "consent_version": "clinic-paper-v2",
                "consent_at": "2026-11-01T09:00:00Z",
                // Not part of the request type: a caller cannot choose the token.
                "participant_token": "child-chosen-by-caller",
            })),
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let token = made["participant_token"].as_str().unwrap();
    assert!(is_participant_token(token), "{token}");
    assert_eq!(made["org_id"], c.org.to_string());
    assert_eq!(made["session_count"], 0);
    assert_eq!(made["age_bracket_at_enrol"], "24-36m");
    assert_eq!(made["language"], "ne");
    // Flattened: the Participant keys sit beside qr_payload; admin may see the MRN flag.
    assert_eq!(made["has_patient_ref"], false);
    let (org, qr_token) = parse_qr_payload(made["qr_payload"].as_str().unwrap()).unwrap();
    assert_eq!((org, qr_token.as_str()), (c.org, token));

    let id: Uuid = made["id"].as_str().unwrap().parse().unwrap();
    let row: (String, Option<String>, bool) = sqlx::query_as(
        "SELECT mode, consent_version, consent_at IS NOT NULL FROM participants WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(row, ("clinic".into(), Some("clinic-paper-v2".into()), true));

    // The list shows it with no sessions.
    let (s, page) = c.call(Method::GET, &create_url(c.org), None, &admin).await;
    assert_eq!(s, StatusCode::OK);
    let listed: Vec<&Value> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["id"] == made["id"])
        .collect();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["session_count"], 0);

    let audit = c.audit_rows("participant.create").await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["target_id"], id.to_string());
    assert_eq!(audit[0]["after"]["has_consent"], true);
    assert!(
        !audit[0].to_string().contains(token),
        "no token in the audit"
    );

    // An empty body is fine; two creates mint two tokens.
    let (s, a) = c.call(Method::POST, &create_url(c.org), None, &admin).await;
    assert_eq!(s, StatusCode::CREATED, "{a}");
    assert_ne!(a["participant_token"], made["participant_token"]);
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_device_session_for_the_created_token_lands_on_the_same_participant() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clinician) = c.token(Role::Clinician).await;
    let (s, made) = c
        .call(
            Method::POST,
            &create_url(c.org),
            Some(json!({})),
            &clinician,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let token = made["participant_token"].as_str().unwrap().to_owned();

    let (_, device) = ingest_support::seed_device(&c.db.pool, c.org).await;
    let mut body = ingest_support::golden_with_id("SESS-EMR-0001");
    body["participantToken"] = json!(token);
    body["rawPayload"]["participantId"] = json!(token);
    ingest_support::ingest_ok(&c.app, &device, &body).await;

    let on: Uuid = sqlx::query_scalar("SELECT participant_id FROM sessions WHERE id = $1")
        .bind("SESS-EMR-0001")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(on.to_string(), made["id"].as_str().unwrap());
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM participants WHERE org_id = $1")
        .bind(c.org)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(n, 1, "no second participant");
    c.db.cleanup().await;
}

#[tokio::test]
async fn create_for_a_token_a_device_already_posted_links_never_conflicts() {
    let Some(c) = Ctx::new().await else { return };
    // The ingest shape: a device-created row has only org and token.
    let existing: Uuid = sqlx::query_scalar(
        "INSERT INTO participants (org_id, token) VALUES ($1, 'child-0123456789abcdef0123')
         RETURNING id",
    )
    .bind(c.org)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    let mut conn = c.db.pool.acquire().await.unwrap();
    let fields = NewParticipant {
        age_bracket_at_enrol: Some("36-48m".into()),
        language: Some("en".into()),
        ..NewParticipant::default()
    };
    let id = upsert_clinic_participant(&mut conn, c.org, "child-0123456789abcdef0123", &fields)
        .await
        .unwrap();
    assert_eq!(id, existing);
    // Fields the device row lacked are filled; a second call overwrites nothing.
    let other = NewParticipant {
        age_bracket_at_enrol: Some("12-24m".into()),
        ..NewParticipant::default()
    };
    let again = upsert_clinic_participant(&mut conn, c.org, "child-0123456789abcdef0123", &other)
        .await
        .unwrap();
    assert_eq!(again, existing);
    let row: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT age_bracket_at_enrol, language, mode FROM participants WHERE id = $1",
    )
    .bind(existing)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            Some("36-48m".into()),
            Some("en".into()),
            Some("clinic".into())
        )
    );
    drop(conn);
    c.db.cleanup().await;
}

#[tokio::test]
async fn roles_validation_and_org_scoping() {
    let Some(c) = Ctx::new().await else { return };
    for role in [Role::Viewer, Role::Researcher, Role::Reviewer] {
        let (_, t) = c.token(role).await;
        let (s, _) = c.call(Method::POST, &create_url(c.org), None, &t).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?}");
    }
    for role in [Role::Owner, Role::Admin, Role::Clinician, Role::Superadmin] {
        let (_, t) = c.token(role).await;
        let (s, b) = c.call(Method::POST, &create_url(c.org), None, &t).await;
        assert_eq!(s, StatusCode::CREATED, "{role:?}: {b}");
        // RBAC-3: the superadmin creates but never sees the MRN flag.
        if role == Role::Superadmin {
            assert!(b.get("has_patient_ref").is_none(), "{b}");
        }
    }
    // Another org's admin: 404.
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, b_admin) = c.token_in(Some(other), Role::Admin).await;
    let (s, _) = c
        .call(Method::POST, &create_url(c.org), None, &b_admin)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (_, admin) = c.token(Role::Admin).await;
    for bad in [
        json!({"consent_at": "yesterday"}),
        json!({"consent_at": "2999-01-01T00:00:00Z"}),
        json!({"language": "x".repeat(65)}),
        json!({"age_bracket_at_enrol": "a\u{0}b"}),
    ] {
        let (s, b) = c
            .call(Method::POST, &create_url(c.org), Some(bad.clone()), &admin)
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}: {b}");
        assert_eq!(slug_of(&b), "validation-failed");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn participant_read_audit_still_fires() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clinician) = c.token(Role::Clinician).await;
    let (_, made) = c
        .call(Method::POST, &create_url(c.org), None, &clinician)
        .await;
    let id = made["id"].as_str().unwrap();
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{id}"),
            None,
            &clinician,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let rows = c.audit_rows("participant.read").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["target_id"], id);
    c.db.cleanup().await;
}

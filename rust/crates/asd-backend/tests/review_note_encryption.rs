//! owner: WP-C.9
//!
//! KEY-7 (G2): `review_items.note` moves to the envelope scheme. From migration 0008 on the
//! API writes only `note_ct`/`note_key_id`; `asd-backend admin encrypt-review-notes`
//! encrypts rows written before 0008 and empties `note`, idempotently. The contract
//! (`CreateReviewRequest.note`) is unchanged. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use std::sync::Arc;

use asd_backend::admin::{self, AdminCommand};
use asd_backend::app::AppState;
use asd_backend::crypto::{Aad, Keyring};
use common::auth::{access_token, json, seed_user};
use common::{send, TestDb};
use http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organizations (name, kind) VALUES ($1, 'clinic') RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A participant and a session `id` in `org`.
async fn session(pool: &PgPool, org: Uuid, id: &str) {
    let token = format!("child-{id}");
    let participant: Uuid =
        sqlx::query_scalar("INSERT INTO participants (org_id, token) VALUES ($1, $2) RETURNING id")
            .bind(org)
            .bind(&token)
            .fetch_one(pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, org_id, participant_id, participant_token, capture_source, \
           started_at, duration_sec, schema_version, quality_verdict, valid_frame_ratio, \
           social_preference, attention_flag, summary) \
         VALUES ($1, $2, $3, $4, 'truedepth', now(), 30, '1.6', 'optimal', 0.9, 0.6, 'green', '{}')",
    )
    .bind(id)
    .bind(org)
    .bind(participant)
    .bind(&token)
    .execute(pool)
    .await
    .unwrap();
}

struct Stored {
    id: Uuid,
    org: Uuid,
    note: Option<String>,
    note_ct: Option<Vec<u8>>,
    note_key_id: Option<Uuid>,
}

async fn stored(pool: &PgPool, session: &str, reason: &str) -> Stored {
    let (id, org, note, note_ct, note_key_id) =
        sqlx::query_as::<_, (Uuid, Uuid, Option<String>, Option<Vec<u8>>, Option<Uuid>)>(
            "SELECT id, org_id, note, note_ct, note_key_id FROM review_items \
         WHERE session_id = $1 AND reason = $2",
        )
        .bind(session)
        .bind(reason)
        .fetch_one(pool)
        .await
        .unwrap();
    Stored {
        id,
        org,
        note,
        note_ct,
        note_key_id,
    }
}

async fn decrypt(keyring: &Keyring, s: &Stored) -> String {
    keyring
        .decrypt_string(
            s.org,
            &Aad::new("review_items", "note_ct", s.org, s.id),
            s.note_key_id.expect("key id"),
            s.note_ct.as_deref().expect("ciphertext"),
        )
        .await
        .unwrap()
}

fn keyring_of(state: &AppState) -> Arc<Keyring> {
    state.keyring.clone().expect("test config carries a KEK")
}

#[tokio::test]
async fn manual_enqueue_stores_only_ciphertext() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let state = common::state_with_pool(db.pool.clone());
    let app = common::router(state.clone());
    let o = org(&db.pool, "Clinic").await;
    session(&db.pool, o, "S-NOTE").await;
    session(&db.pool, o, "S-PLAIN").await;
    let clinician = seed_user(
        &db.pool,
        Some(o),
        asd_contract::api_v1::Role::Clinician,
        "c@example.org",
    )
    .await;
    let token = access_token(
        &state,
        clinician,
        Some(o),
        asd_contract::api_v1::Role::Clinician,
    );
    let uri = format!("/v1/orgs/{o}/review");

    let note = "second look: child turned away at clip 4";
    let (status, _, body) = send(
        app.clone(),
        json(
            Method::POST,
            &uri,
            &json!({"session_id": "S-NOTE", "note": note}),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(
        body.get("note").is_none(),
        "ReviewItem never returns the note"
    );
    let s = stored(&db.pool, "S-NOTE", "manual").await;
    assert_eq!(s.note, None, "plaintext column stays NULL");
    assert_eq!(s.id.to_string(), body["id"].as_str().unwrap());
    let ct = s.note_ct.as_deref().unwrap();
    assert!(
        !ct.windows(6).any(|w| w == b"second"),
        "no plaintext in the ciphertext"
    );
    assert_eq!(decrypt(&keyring_of(&state), &s).await, note);

    // No note: nothing to encrypt.
    let (status, _, body) = send(
        app.clone(),
        json(
            Method::POST,
            &uri,
            &json!({"session_id": "S-PLAIN"}),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let s = stored(&db.pool, "S-PLAIN", "manual").await;
    assert!(s.note.is_none() && s.note_ct.is_none() && s.note_key_id.is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn without_a_kek_a_note_is_refused_but_an_empty_enqueue_works() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let mut state = common::state_with_pool(db.pool.clone());
    state.keyring = None;
    let app = common::router(state.clone());
    let o = org(&db.pool, "Clinic").await;
    session(&db.pool, o, "S-1").await;
    let clinician = seed_user(
        &db.pool,
        Some(o),
        asd_contract::api_v1::Role::Clinician,
        "c@example.org",
    )
    .await;
    let token = access_token(
        &state,
        clinician,
        Some(o),
        asd_contract::api_v1::Role::Clinician,
    );
    let uri = format!("/v1/orgs/{o}/review");

    let (status, _, body) = send(
        app.clone(),
        json(
            Method::POST,
            &uri,
            &json!({"session_id": "S-1", "note": "x"}),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(
        body["type"],
        "https://schemas.curv.org/asd/problems/clinical-crypto-unavailable"
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM review_items")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "nothing stored, in clear or otherwise");

    let (status, _, body) = send(
        app,
        json(
            Method::POST,
            &uri,
            &json!({"session_id": "S-1", "note": "  "}),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "a blank note is no note: {body}"
    );
    db.cleanup().await;
}

/// A row as written by the pre-0008 code path: plaintext `note`, no ciphertext.
async fn legacy_row(pool: &PgPool, org: Uuid, session_id: &str, note: Option<&str>) {
    sqlx::query(
        "INSERT INTO review_items (org_id, session_id, reason, note) VALUES ($1, $2, 'manual', $3)",
    )
    .bind(org)
    .bind(session_id)
    .bind(note)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn backfill_encrypts_legacy_notes_and_is_idempotent() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let state = common::state_with_pool(db.pool.clone());
    let keyring = keyring_of(&state);
    let a = org(&db.pool, "Clinic A").await;
    let b = org(&db.pool, "Clinic B").await;
    let legacy = [
        (a, "S-A1", Some("note one")),
        (a, "S-A2", Some("note two, नेपाल")),
        (b, "S-B1", Some("note three")),
        (b, "S-B2", None),
    ];
    for (o, s, note) in legacy {
        session(&db.pool, o, s).await;
        legacy_row(&db.pool, o, s, note).await;
    }
    let audit_before: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(&db.pool)
        .await
        .unwrap();

    // Batch size 2 forces more than one batch.
    let changed = admin::encrypt_review_notes(&db.pool, &keyring, 2)
        .await
        .unwrap();
    assert_eq!(changed, 3);
    let mut ciphertexts = Vec::new();
    for (_, s, note) in legacy {
        let row = stored(&db.pool, s, "manual").await;
        assert_eq!(row.note, None, "{s}: plaintext emptied");
        match note {
            Some(text) => {
                assert_eq!(decrypt(&keyring, &row).await, text, "{s}");
                ciphertexts.push(row.note_ct.clone());
            }
            None => assert!(row.note_ct.is_none(), "{s}: nothing invented"),
        }
    }
    let audit_rows: Vec<(Option<Uuid>, serde_json::Value)> = sqlx::query_as(
        "SELECT org_id, after FROM audit_log WHERE action = 'review.note_encrypt' ORDER BY id",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert!(!audit_rows.is_empty());
    let total: i64 = audit_rows
        .iter()
        .map(|(_, after)| after["encrypted"].as_i64().unwrap())
        .sum();
    assert_eq!(total, 3, "audit rows account for every row");
    assert!(
        audit_rows.iter().all(|(org, _)| org.is_some()),
        "audit rows are per org"
    );
    assert!(
        !audit_rows
            .iter()
            .any(|(_, after)| after.to_string().contains("note one")),
        "no note text in the audit log"
    );

    // Second run: nothing to do, nothing changes, no further audit rows.
    let audit_mid: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(audit_mid > audit_before);
    assert_eq!(
        admin::encrypt_review_notes(&db.pool, &keyring, 2)
            .await
            .unwrap(),
        0
    );
    let mut again = Vec::new();
    for (_, s, note) in legacy {
        if note.is_some() {
            again.push(stored(&db.pool, s, "manual").await.note_ct);
        }
    }
    assert_eq!(
        again, ciphertexts,
        "ciphertexts untouched by the second run"
    );
    let audit_after: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(audit_after, audit_mid);
    db.cleanup().await;
}

#[tokio::test]
async fn backfill_clears_plaintext_left_beside_existing_ciphertext() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let state = common::state_with_pool(db.pool.clone());
    let keyring = keyring_of(&state);
    let o = org(&db.pool, "Clinic").await;
    session(&db.pool, o, "S-1").await;
    legacy_row(&db.pool, o, "S-1", Some("original")).await;
    assert_eq!(
        admin::encrypt_review_notes(&db.pool, &keyring, 10)
            .await
            .unwrap(),
        1
    );
    let first = stored(&db.pool, "S-1", "manual").await;
    // Someone writes plaintext again (an old API replica during a rolling deploy).
    sqlx::query("UPDATE review_items SET note = 'stale copy' WHERE id = $1")
        .bind(first.id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        admin::encrypt_review_notes(&db.pool, &keyring, 10)
            .await
            .unwrap(),
        1
    );
    let second = stored(&db.pool, "S-1", "manual").await;
    assert_eq!(second.note, None);
    assert_eq!(second.note_ct, first.note_ct, "existing ciphertext wins");
    db.cleanup().await;
}

#[test]
fn command_parses() {
    let args = |s: &[&str]| s.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        admin::parse(&args(&["encrypt-review-notes"])).unwrap(),
        AdminCommand::EncryptReviewNotes { batch_size: 500 }
    );
    assert_eq!(
        admin::parse(&args(&["encrypt-review-notes", "--batch-size", "50"])).unwrap(),
        AdminCommand::EncryptReviewNotes { batch_size: 50 }
    );
    for bad in [
        &["encrypt-review-notes", "--batch-size", "0"][..],
        &["encrypt-review-notes", "--batch-size", "x"],
        &["encrypt-review-notes", "--force"],
    ] {
        assert_eq!(
            admin::parse(&args(bad)).unwrap_err().exit_code(),
            2,
            "{bad:?}"
        );
    }
}

//! owner: WP-C.9
//!
//! KEY-7 (G2): review notes use the envelope scheme. From migration 0008 on the API writes
//! only `note_ct`/`note_key_id`; the plaintext `review_items.note` column and its
//! `admin encrypt-review-notes` back-fill were removed by 0022 (WP-B.14, KEY-11; the guard
//! is tested in `migrations.rs`). The contract (`CreateReviewRequest.note`) is unchanged.
//! Needs `DATABASE_URL`; skipped otherwise.

mod common;

use std::sync::Arc;

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
    note_ct: Option<Vec<u8>>,
    note_key_id: Option<Uuid>,
}

async fn stored(pool: &PgPool, session: &str, reason: &str) -> Stored {
    let (id, org, note_ct, note_key_id) =
        sqlx::query_as::<_, (Uuid, Uuid, Option<Vec<u8>>, Option<Uuid>)>(
            "SELECT id, org_id, note_ct, note_key_id FROM review_items \
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
    assert!(s.note_ct.is_none() && s.note_key_id.is_none());
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

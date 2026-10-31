//! owner: WP-C.11
//!
//! Clinical notes (NOTE-1, NOTE-2): encrypted at rest, append-only (a correction is a new
//! row, the original is untouched), role-gated, org-scoped, and the body never reaches the
//! timeline, the audit log or the logs. Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod participant_support;

use asd_contract::api_v1::Role;
use asd_contract::clinical::{ClinicalNote, ClinicalNotePage};
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::json;
use uuid::Uuid;

const SECRET: &str = "Parent reports rocking and no pointing, see Dr Rai";

fn notes_url(p: impl std::fmt::Display) -> String {
    format!("/v1/participants/{p}/notes")
}

#[tokio::test]
async fn create_list_get_round_trip_with_encrypted_body() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    c.session(c.org, p, "S-1", "2026-10-01T10:00:00Z").await;
    let (author, clin) = c.token(Role::Clinician).await;
    sqlx::query("UPDATE users SET display_name = 'Dr. Sita Rai' WHERE id = $1")
        .bind(author)
        .execute(&c.db.pool)
        .await
        .unwrap();

    let (s, made) = c
        .call(
            Method::POST,
            &notes_url(p),
            Some(json!({"body": SECRET, "session_id": "S-1"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let note: ClinicalNote = serde_json::from_value(made).unwrap();
    assert_eq!(note.body.as_deref(), Some(SECRET));
    assert_eq!(note.kind.as_str(), "general");
    assert_eq!(note.session_id.as_deref(), Some("S-1"));
    assert_eq!(note.author_id, author.to_string());
    assert_eq!(note.author_display_name.as_deref(), Some("Dr. Sita Rai"));

    let ct: Vec<u8> = sqlx::query_scalar("SELECT body_ct FROM clinical_notes WHERE id = $1")
        .bind(note.id.parse::<Uuid>().unwrap())
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&ct).contains("rocking"));

    let (s, one) = c
        .call(Method::GET, &format!("/v1/notes/{}", note.id), None, &clin)
        .await;
    assert_eq!(s, StatusCode::OK, "{one}");
    assert_eq!(one["body"], SECRET);

    let (s, page) = c.call(Method::GET, &notes_url(p), None, &clin).await;
    assert_eq!(s, StatusCode::OK, "{page}");
    let page: ClinicalNotePage = serde_json::from_value(page).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.total, Some(1));
    assert_eq!(page.items[0].body.as_deref(), Some(SECRET));

    // The audit row names the note but never its text.
    let rows = c.audit_rows("note.create").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["target_type"], "clinical_note");
    assert!(!c.audit_dump().await.contains("rocking"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn correction_appends_a_row_and_leaves_the_original_unchanged() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, first) = c
        .call(
            Method::POST,
            &notes_url(p),
            Some(json!({"body": "first"})),
            &clin,
        )
        .await;
    let first_id = first["id"].as_str().unwrap().to_owned();
    let before: (Vec<u8>, Uuid, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT body_ct, body_key_id, created_at FROM clinical_notes WHERE id = $1")
            .bind(first_id.parse::<Uuid>().unwrap())
            .fetch_one(&c.db.pool)
            .await
            .unwrap();

    let (s, fix) = c
        .call(
            Method::POST,
            &notes_url(p),
            Some(json!({"body": "second, corrected", "corrects_id": first_id})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{fix}");
    assert_eq!(fix["kind"], "correction");
    assert_eq!(fix["corrects_id"], first_id);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM clinical_notes")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    let after: (Vec<u8>, Uuid, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT body_ct, body_key_id, created_at FROM clinical_notes WHERE id = $1")
            .bind(first_id.parse::<Uuid>().unwrap())
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(before, after);

    // The original reports who corrected it; its text is unchanged.
    let (_, one) = c
        .call(Method::GET, &format!("/v1/notes/{first_id}"), None, &clin)
        .await;
    assert_eq!(one["body"], "first");
    assert_eq!(one["corrected_by"], fix["id"]);
    assert!(one["corrects_id"].is_null());

    // Newest first.
    let (_, page) = c.call(Method::GET, &notes_url(p), None, &clin).await;
    assert_eq!(page["items"][0]["id"], fix["id"]);
    assert_eq!(page["items"][1]["id"], first_id.as_str());

    // There is no edit or delete route, and the database refuses both.
    for method in [Method::PATCH, Method::PUT, Method::DELETE] {
        let (s, _) = c
            .call(
                method.clone(),
                &format!("/v1/notes/{first_id}"),
                Some(json!({"body": "x"})),
                &clin,
            )
            .await;
        assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED, "{method}");
    }
    assert!(sqlx::query("UPDATE clinical_notes SET kind = 'general'")
        .execute(&c.db.pool)
        .await
        .is_err());
    c.db.cleanup().await;
}

#[tokio::test]
async fn note_validation() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let other = c.participant(c.org, "P-2").await;
    c.session(c.org, other, "S-other", "2026-10-01T10:00:00Z")
        .await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, theirs) = c
        .call(
            Method::POST,
            &notes_url(other),
            Some(json!({"body": "x"})),
            &clin,
        )
        .await;

    let too_long = "x".repeat(20_001);
    for body in [
        json!({"body": ""}),
        json!({"body": "   "}),
        json!({"body": too_long}),
        json!({}),
        // a session of another participant
        json!({"body": "x", "session_id": "S-other"}),
        json!({"body": "x", "session_id": "nope"}),
        // a note of another participant, or none at all
        json!({"body": "x", "corrects_id": theirs["id"]}),
        json!({"body": "x", "corrects_id": Uuid::new_v4().to_string()}),
        json!({"body": "x", "corrects_id": "not-a-uuid"}),
    ] {
        let (s, resp) = c
            .call(Method::POST, &notes_url(p), Some(body.clone()), &clin)
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {resp}");
        assert_eq!(slug_of(&resp), "validation-failed");
    }
    let exactly = "x".repeat(20_000);
    let (s, _) = c
        .call(
            Method::POST,
            &notes_url(p),
            Some(json!({"body": exactly})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, _) = c
        .call(
            Method::POST,
            &notes_url(Uuid::new_v4()),
            Some(json!({"body": "x"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn roles_and_org_scoping() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, made) = c
        .call(
            Method::POST,
            &notes_url(p),
            Some(json!({"body": SECRET})),
            &clin,
        )
        .await;
    let id = made["id"].as_str().unwrap();

    // Owner reads (ClinicalContentRead) but cannot write (ClinicalNoteWrite).
    let (_, owner) = c.token(Role::Owner).await;
    let (s, _) = c.call(Method::GET, &notes_url(p), None, &owner).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = c
        .call(
            Method::POST,
            &notes_url(p),
            Some(json!({"body": "x"})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // Admin, researcher, reviewer, viewer and superadmin see no clinical content.
    for role in [
        Role::Admin,
        Role::Researcher,
        Role::Reviewer,
        Role::Viewer,
        Role::Superadmin,
    ] {
        let (_, t) = c.token(role).await;
        for (method, uri, body) in [
            (Method::GET, notes_url(p), None),
            (Method::GET, format!("/v1/notes/{id}"), None),
            (Method::POST, notes_url(p), Some(json!({"body": "x"}))),
        ] {
            let (s, resp) = c.call(method.clone(), &uri, body, &t).await;
            assert_eq!(s, StatusCode::FORBIDDEN, "{role:?} {method} {uri}");
            assert!(!resp.to_string().contains("rocking"));
        }
    }

    // Another org's clinician gets 404, never the note.
    let org2 = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, foreign) = c.token_in(Some(org2), Role::Clinician).await;
    for (method, uri, body) in [
        (Method::GET, notes_url(p), None),
        (Method::GET, format!("/v1/notes/{id}"), None),
        (Method::POST, notes_url(p), Some(json!({"body": "x"}))),
    ] {
        let (s, _) = c.call(method.clone(), &uri, body, &foreign).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{method} {uri}");
    }
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/notes/{}", Uuid::new_v4()),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn pagination_is_newest_first_with_a_stable_cursor_across_ties() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let (author, clin) = c.token(Role::Clinician).await;
    let keyring = c.state.keyring.clone().expect("test state has a keyring");
    // Five notes with one created_at, so the page boundary falls inside a tie.
    let mut ids = Vec::new();
    for i in 0..5 {
        let id = Uuid::new_v4();
        let aad = asd_backend::crypto::Aad::new("clinical_notes", "body_ct", c.org, id);
        let sealed = keyring
            .encrypt_str(c.org, &aad, &format!("note {i}"))
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO clinical_notes (id, org_id, participant_id, body_ct, body_key_id, author_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, '2030-01-01T00:00:00Z')",
        )
        .bind(id)
        .bind(c.org)
        .bind(p)
        .bind(sealed.ciphertext)
        .bind(sealed.key_id)
        .bind(author)
        .execute(&c.db.pool)
        .await
        .unwrap();
        ids.push(id.to_string());
    }
    ids.sort();
    ids.reverse(); // (created_at DESC, id DESC)

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut uri = format!("{}?limit=2", notes_url(p));
        if let Some(cur) = &cursor {
            uri.push_str(&format!("&cursor={cur}"));
        }
        let (s, page) = c.call(Method::GET, &uri, None, &clin).await;
        assert_eq!(s, StatusCode::OK, "{page}");
        for item in page["items"].as_array().unwrap() {
            seen.push(item["id"].as_str().unwrap().to_owned());
        }
        assert_eq!(page["total"], 5);
        match page["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    assert_eq!(seen, ids);
    let (s, _) = c
        .call(
            Method::GET,
            &format!("{}?limit=0", notes_url(p)),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    c.db.cleanup().await;
}

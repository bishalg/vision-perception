//! owner: WP-C.11
//!
//! Participant detail and timeline (CW-3): `GET /v1/participants/{id}`, and the merged,
//! newest-first, cursor-paged stream of sessions, clinical reviews and signatures, note
//! and letter headers and care events. Bodies never appear in it. Needs `DATABASE_URL`;
//! skipped otherwise.

mod common;
mod participant_support;

use asd_contract::api_v1::{Participant, Role};
use asd_contract::clinical::{TimelineEntryKind, TimelinePage};
use http::{Method, StatusCode};
use participant_support::*;
use uuid::Uuid;

const NOTE_TEXT: &str = "SECRET-NOTE-TEXT";
const LETTER_TEXT: &str = "SECRET-LETTER-TEXT";

async fn seed_note(c: &Ctx, p: Uuid, author: Uuid, at: &str, text: &str) -> Uuid {
    let id = Uuid::new_v4();
    let aad = asd_backend::crypto::Aad::new("clinical_notes", "body_ct", c.org, id);
    let sealed = c
        .state
        .keyring
        .clone()
        .unwrap()
        .encrypt_str(c.org, &aad, text)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO clinical_notes (id, org_id, participant_id, body_ct, body_key_id, author_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz)",
    )
    .bind(id)
    .bind(c.org)
    .bind(p)
    .bind(sealed.ciphertext)
    .bind(sealed.key_id)
    .bind(author)
    .bind(at)
    .execute(&c.db.pool)
    .await
    .unwrap();
    id
}

async fn seed_care(c: &Ctx, p: Uuid, at: &str, kind: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO care_events (org_id, participant_id, kind, occurred_at, payload)
         VALUES ($1, $2, $3, $4::timestamptz, '{\"text\": \"SECRET-DIARY\"}') RETURNING id",
    )
    .bind(c.org)
    .bind(p)
    .bind(kind)
    .bind(at)
    .fetch_one(&c.db.pool)
    .await
    .unwrap()
}

async fn seed_letter(c: &Ctx, p: Uuid, author: Uuid, at: &str) -> Uuid {
    let id = Uuid::new_v4();
    let aad = asd_backend::crypto::Aad::new("referral_letters", "body_ct", c.org, id);
    let sealed = c
        .state
        .keyring
        .clone()
        .unwrap()
        .encrypt_str(c.org, &aad, LETTER_TEXT)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO referral_letters (id, org_id, participant_id, kind, body_ct, body_key_id, author_id, created_at)
         VALUES ($1, $2, $3, 'referral', $4, $5, $6, $7::timestamptz)",
    )
    .bind(id)
    .bind(c.org)
    .bind(p)
    .bind(sealed.ciphertext)
    .bind(sealed.key_id)
    .bind(author)
    .bind(at)
    .execute(&c.db.pool)
    .await
    .unwrap();
    id
}

/// An open review of `session` created at `at`, signed at `signed` by `signer`.
async fn seed_signed_review(
    c: &Ctx,
    session: &str,
    signer: Uuid,
    created: &str,
    signed: &str,
) -> (Uuid, Uuid) {
    let review: Uuid = sqlx::query_scalar(
        "INSERT INTO clinical_reviews (org_id, session_id, state, created_by, created_at)
         VALUES ($1, $2, 'final', $3, $4::timestamptz) RETURNING id",
    )
    .bind(c.org)
    .bind(session)
    .bind(signer)
    .bind(created)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    let version: Uuid = sqlx::query_scalar(
        "INSERT INTO clinical_review_versions
            (review_id, org_id, version_no, status, disposition, content_hash, signed_at)
         VALUES ($1, $2, 1, 'signed', 'monitor', decode(repeat('ab', 32), 'hex'), $3::timestamptz)
         RETURNING id",
    )
    .bind(review)
    .bind(c.org)
    .bind(signed)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    let sig: Uuid = sqlx::query_scalar(
        "INSERT INTO clinical_signatures
            (org_id, review_id, version_id, signer_id, role, meaning, content_hash,
             signer_display_name, signed_at)
         VALUES ($1, $2, $3, $4, 'sign', 'm', decode(repeat('ab', 32), 'hex'), 'Dr. A', $5::timestamptz)
         RETURNING id",
    )
    .bind(c.org)
    .bind(review)
    .bind(version)
    .bind(signer)
    .bind(signed)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    (review, sig)
}

async fn timeline(c: &Ctx, p: Uuid, query: &str, bearer: &str) -> TimelinePage {
    let (s, body) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{p}/timeline{query}"),
            None,
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    serde_json::from_value(body).unwrap()
}

#[tokio::test]
async fn participant_detail_returns_the_participant() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    c.session(c.org, p, "S-1", "2026-10-01T10:00:00Z").await;
    c.session(c.org, p, "S-2", "2026-10-02T10:00:00Z").await;
    let (_, viewer) = c.token(Role::Viewer).await;

    let (s, body) = c
        .call(Method::GET, &format!("/v1/participants/{p}"), None, &viewer)
        .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let part: Participant = serde_json::from_value(body).unwrap();
    assert_eq!(part.id, p.to_string());
    assert_eq!(part.org_id, c.org.to_string());
    assert_eq!(part.participant_token, "P-1");
    assert_eq!(part.session_count, 2);
    assert_eq!(
        part.last_session_at.as_deref(),
        Some("2026-10-02T10:00:00.000Z")
    );
    assert_eq!(part.language.as_deref(), Some("en"));

    // Unknown, malformed and foreign ids are all 404.
    let org2 = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, foreign) = c.token_in(Some(org2), Role::Clinician).await;
    for (uri, bearer) in [
        (format!("/v1/participants/{}", Uuid::new_v4()), &viewer),
        ("/v1/participants/not-a-uuid".to_owned(), &viewer),
        (format!("/v1/participants/{p}"), &foreign),
        (format!("/v1/participants/{p}/timeline"), &foreign),
    ] {
        let (s, _) = c.call(Method::GET, &uri, None, bearer).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{uri}");
    }
    // Superadmin may read the pseudonymous participant (ParticipantRead is not clinical).
    let (_, root) = c.token(Role::Superadmin).await;
    let (s, _) = c
        .call(Method::GET, &format!("/v1/participants/{p}"), None, &root)
        .await;
    assert_eq!(s, StatusCode::OK);
    c.db.cleanup().await;
}

#[tokio::test]
async fn timeline_merges_all_five_kinds_newest_first_without_bodies() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let (author, _clin) = c.token(Role::Clinician).await;
    sqlx::query("UPDATE users SET display_name = 'Dr. Sita Rai' WHERE id = $1")
        .bind(author)
        .execute(&c.db.pool)
        .await
        .unwrap();
    // Another participant's rows must not leak in.
    let q = c.participant(c.org, "P-2").await;
    c.session(c.org, q, "S-other", "2026-10-03T00:00:00Z").await;
    seed_care(&c, q, "2026-10-03T00:00:00Z", "diary").await;

    c.session(c.org, p, "S-1", "2026-10-01T10:00:00Z").await; // t1
    let (review, sig) = seed_signed_review(
        &c,
        "S-1",
        author,
        "2026-10-01T11:00:00Z", // t2 opened
        "2026-10-01T12:00:00Z", // t3 signed
    )
    .await;
    let note = seed_note(&c, p, author, "2026-10-01T13:00:00Z", NOTE_TEXT).await; // t4
    let letter = seed_letter(&c, p, author, "2026-10-01T14:00:00Z").await; // t5
    let care = seed_care(&c, p, "2026-10-01T15:00:00Z", "diary").await; // t6

    // Even a viewer sees the timeline (ParticipantRead), as headers only.
    let (_, viewer) = c.token(Role::Viewer).await;
    let page = timeline(&c, p, "", &viewer).await;
    assert!(page.next_cursor.is_none());
    let got: Vec<(TimelineEntryKind, String)> =
        page.items.iter().map(|e| (e.kind, e.id.clone())).collect();
    assert_eq!(
        got,
        vec![
            (TimelineEntryKind::CareEvent, care.to_string()),
            (TimelineEntryKind::Letter, letter.to_string()),
            (TimelineEntryKind::Note, note.to_string()),
            (TimelineEntryKind::ClinicalReview, sig.to_string()),
            (TimelineEntryKind::ClinicalReview, review.to_string()),
            (TimelineEntryKind::Session, "S-1".to_owned()),
        ]
    );
    let by_kind = |k: TimelineEntryKind| page.items.iter().find(|e| e.kind == k).unwrap();
    assert_eq!(
        by_kind(TimelineEntryKind::Session).summary["attention_flag"],
        "red"
    );
    assert_eq!(
        by_kind(TimelineEntryKind::Note).summary["note_kind"],
        "general"
    );
    assert_eq!(
        by_kind(TimelineEntryKind::Note)
            .actor_display_name
            .as_deref(),
        Some("Dr. Sita Rai")
    );
    assert_eq!(
        by_kind(TimelineEntryKind::Letter).summary["letter_kind"],
        "referral"
    );
    assert_eq!(
        by_kind(TimelineEntryKind::Letter).summary["status"],
        "draft"
    );
    assert_eq!(
        by_kind(TimelineEntryKind::CareEvent).summary["care_kind"],
        "diary"
    );
    let signed = page.items.iter().find(|e| e.id == sig.to_string()).unwrap();
    assert_eq!(signed.summary["event"], "signed");
    assert_eq!(signed.summary["version_no"], 1);
    assert_eq!(signed.session_id.as_deref(), Some("S-1"));
    let opened = page
        .items
        .iter()
        .find(|e| e.id == review.to_string())
        .unwrap();
    assert_eq!(opened.summary["event"], "opened");
    assert_eq!(opened.summary["state"], "final");

    let raw = serde_json::to_string(&page).unwrap();
    for secret in [NOTE_TEXT, LETTER_TEXT, "SECRET-DIARY"] {
        assert!(!raw.contains(secret), "timeline holds {secret}");
    }
    assert!(!raw.contains("P-2") && !raw.contains("S-other"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn cursor_is_stable_across_pages_with_equal_timestamps() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let (author, clin) = c.token(Role::Clinician).await;
    // Seven entries of four kinds on one instant, plus two older ones.
    let at = "2026-10-05T09:00:00Z";
    for i in 0..3 {
        c.session(c.org, p, &format!("S-tie-{i}"), at).await;
    }
    for _ in 0..2 {
        seed_note(&c, p, author, at, NOTE_TEXT).await;
    }
    for _ in 0..2 {
        seed_care(&c, p, at, "task").await;
    }
    c.session(c.org, p, "S-old", "2026-10-01T00:00:00Z").await;
    seed_care(&c, p, "2026-09-01T00:00:00Z", "milestone").await;

    let all = timeline(&c, p, "?limit=100", &clin).await;
    assert_eq!(all.items.len(), 9);
    assert!(all.next_cursor.is_none());
    let expected: Vec<String> = all.items.iter().map(|e| e.id.clone()).collect();
    let mut unique = expected.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 9);
    // Non-increasing in time.
    assert!(all.items.windows(2).all(|w| w[0].at >= w[1].at));

    for limit in [1, 2, 4] {
        let mut seen = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0;
        loop {
            let q = match &cursor {
                Some(cur) => format!("?limit={limit}&cursor={cur}"),
                None => format!("?limit={limit}"),
            };
            let page = timeline(&c, p, &q, &clin).await;
            assert!(page.items.len() <= limit);
            seen.extend(page.items.iter().map(|e| e.id.clone()));
            pages += 1;
            assert!(pages < 20, "cursor does not advance");
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(seen, expected, "limit {limit}");
    }

    let (s, body) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{p}/timeline?cursor=garbage"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{p}/timeline?limit=101"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    c.db.cleanup().await;
}

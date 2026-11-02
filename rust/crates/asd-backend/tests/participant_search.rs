//! owner: WP-B.19
//!
//! `POST /v1/orgs/{id}/participants/search` (REC-2, MRN-4; acceptance 5): token prefix, age
//! bracket, last session range and open follow-up filters; the record counters; MRN search
//! only with `PatientRefRead`, only in `mrn` mode and only in the body; one
//! `participant.search` audit row per page naming the ids. Needs `DATABASE_URL`.

mod common;
mod participant_support;

use asd_contract::api_v1::Role;
use asd_contract::emr::ParticipantSearchPage;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn url(org: Uuid) -> String {
    format!("/v1/orgs/{org}/participants/search")
}

async fn search(c: &Ctx, body: Value, bearer: &str) -> ParticipantSearchPage {
    let (s, b) = c.call(Method::POST, &url(c.org), Some(body), bearer).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    serde_json::from_value(b).unwrap()
}

fn tokens(page: &ParticipantSearchPage) -> Vec<String> {
    let mut t: Vec<String> = page
        .items
        .iter()
        .map(|r| r.participant.participant_token.clone())
        .collect();
    t.sort();
    t
}

struct Seeded {
    alpha: Uuid,
    beta: Uuid,
}

async fn seed(c: &Ctx, bearer: &str) -> Seeded {
    let alpha = c.participant(c.org, "child-a1a1a1a1a1a1a1a1a1a1").await;
    let beta = c.participant(c.org, "child-b2b2b2b2b2b2b2b2b2b2").await;
    let gamma = c.participant(c.org, "child-a1ffffffffffffffffff").await;
    sqlx::query("UPDATE participants SET age_bracket_at_enrol = '36-48m' WHERE id = $1")
        .bind(beta)
        .execute(&c.db.pool)
        .await
        .unwrap();
    c.session(c.org, alpha, "SESS-S-1", "2026-10-01T10:00:00Z")
        .await;
    c.session(c.org, alpha, "SESS-S-2", "2026-10-15T10:00:00Z")
        .await;
    c.session(c.org, gamma, "SESS-S-3", "2026-08-01T10:00:00Z")
        .await;
    // Beta: one open encounter with a follow-up due.
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/participants/{beta}/encounters"),
            Some(
                json!({"kind": "screening_visit", "occurred_at": "2026-10-20T08:00:00Z",
                        "follow_up_due_at": "2026-12-01T00:00:00Z"}),
            ),
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    Seeded { alpha, beta }
}

#[tokio::test]
async fn filters_counters_and_audit() {
    let Some(c) = Ctx::new().await else { return };
    let (_, admin) = c.token(Role::Admin).await;
    let s = seed(&c, &admin).await;

    let all = search(&c, json!({}), &admin).await;
    assert_eq!(all.total, Some(3));

    let by_prefix = search(&c, json!({"token_prefix": "child-a1"}), &admin).await;
    assert_eq!(
        tokens(&by_prefix),
        ["child-a1a1a1a1a1a1a1a1a1a1", "child-a1ffffffffffffffffff"]
    );
    // LIKE wildcards are plain characters in a prefix.
    let none = search(&c, json!({"token_prefix": "child-%"}), &admin).await;
    assert!(none.items.is_empty());

    let by_age = search(&c, json!({"age_bracket": "36-48m"}), &admin).await;
    assert_eq!(tokens(&by_age), ["child-b2b2b2b2b2b2b2b2b2b2"]);

    let follow = search(&c, json!({"has_open_follow_up": true}), &admin).await;
    assert_eq!(follow.items.len(), 1);
    let row = &follow.items[0];
    assert_eq!(row.participant.id, s.beta.to_string());
    assert_eq!(row.open_encounters_n, 1);
    assert_eq!(
        row.next_follow_up_at.as_deref(),
        Some("2026-12-01T00:00:00.000Z")
    );
    let no_follow = search(&c, json!({"has_open_follow_up": false}), &admin).await;
    assert_eq!(no_follow.total, Some(2));

    let seen = search(
        &c,
        json!({"last_seen_from": "2026-10-10", "last_seen_to": "2026-10-31T00:00:00Z"}),
        &admin,
    )
    .await;
    assert_eq!(seen.items.len(), 1);
    let alpha = &seen.items[0].participant;
    assert_eq!(alpha.id, s.alpha.to_string());
    assert_eq!(alpha.session_count, 2);
    assert_eq!(
        alpha.last_session_at.as_deref(),
        Some("2026-10-15T10:00:00.000Z")
    );

    // One audit row per page, ids and filter names only.
    let rows = c.audit_rows("participant.search").await;
    assert_eq!(rows.len(), 7);
    let last = &rows[6];
    assert_eq!(last["target_id"], c.org.to_string());
    assert_eq!(
        last["after"]["participant_ids"],
        json!([s.alpha.to_string()])
    );
    assert_eq!(last["after"]["filters"], json!(["last_seen"]));
    assert!(!c.audit_dump().await.contains("child-a1"));

    // Bad input.
    for bad in [
        json!({"last_seen_from": "last week"}),
        json!({"limit": 0}),
        json!({"limit": 101}),
        json!({"cursor": "not-a-cursor"}),
        json!({"token_prefix": "x".repeat(65)}),
    ] {
        let (st, b) = c
            .call(Method::POST, &url(c.org), Some(bad.clone()), &admin)
            .await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{bad}: {b}");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn paging_is_stable() {
    let Some(c) = Ctx::new().await else { return };
    let (_, viewer) = c.token(Role::Viewer).await;
    for i in 0..5 {
        c.participant(c.org, &format!("child-{i:020}")).await;
    }
    let mut seen = Vec::new();
    let mut body = json!({"limit": 2});
    loop {
        let page = search(&c, body.clone(), &viewer).await;
        assert_eq!(page.total, Some(5));
        // Viewer: no MRN flag.
        assert!(page
            .items
            .iter()
            .all(|r| r.participant.has_patient_ref.is_none()));
        seen.extend(page.items.iter().map(|r| r.participant.id.clone()));
        match page.next_cursor {
            Some(cur) => body = json!({"limit": 2, "cursor": cur}),
            None => break,
        }
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 5);
    assert_eq!(seen.len(), 5);
    c.db.cleanup().await;
}

#[tokio::test]
async fn mrn_search_needs_permission_and_mrn_mode() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, viewer) = c.token(Role::Viewer).await;
    let p = c.participant(c.org, "child-c3c3c3c3c3c3c3c3c3c3").await;
    c.participant(c.org, "child-d4d4d4d4d4d4d4d4d4d4").await;

    // Not in mrn mode: 409.
    let (s, b) = c
        .call(
            Method::POST,
            &url(c.org),
            Some(json!({"mrn": "TU-10042"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug_of(&b), "patient-ref-disabled");

    c.set_mode(c.org, "mrn").await;
    let (s, b) = c
        .call(
            Method::PUT,
            &format!("/v1/participants/{p}/patient-ref"),
            Some(json!({"assigning_authority": "tuth", "mrn": "TU-10042"})),
            &clin,
        )
        .await;
    assert!(s.is_success(), "{b}");

    // Normalised match (case and separators), any authority on file.
    let found = search(&c, json!({"mrn": "tu 10042"}), &clin).await;
    assert_eq!(found.items.len(), 1);
    assert_eq!(found.items[0].participant.id, p.to_string());
    assert_eq!(found.items[0].participant.has_patient_ref, Some(true));
    let missing = search(&c, json!({"mrn": "TU-99999"}), &clin).await;
    assert!(missing.items.is_empty());
    // Combined with another filter.
    let both = search(
        &c,
        json!({"mrn": "TU-10042", "token_prefix": "child-d4"}),
        &clin,
    )
    .await;
    assert!(both.items.is_empty());

    // Viewer and superadmin may search, but not by MRN.
    let (_, root) = c.token(Role::Superadmin).await;
    for t in [&viewer, &root] {
        let (s, _) = c
            .call(
                Method::POST,
                &url(c.org),
                Some(json!({"mrn": "TU-10042"})),
                t,
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        let (s, _) = c.call(Method::POST, &url(c.org), Some(json!({})), t).await;
        assert_eq!(s, StatusCode::OK);
    }
    // The MRN never reaches the audit log.
    let dump = c.audit_dump().await;
    assert!(!dump.contains("10042"), "MRN leaked into audit");
    let rows = c.audit_rows("participant.search").await;
    assert!(rows.iter().any(|r| r["after"]["filters"] == json!(["mrn"])));

    // Another org: 404.
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, b_admin) = c.token_in(Some(other), Role::Admin).await;
    let (s, _) = c
        .call(Method::POST, &url(c.org), Some(json!({})), &b_admin)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

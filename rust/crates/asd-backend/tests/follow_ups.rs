//! owner: WP-B.19
//!
//! `GET /v1/orgs/{id}/follow-ups` (REC-4, acceptance 3): open follow-ups earliest due
//! first; marking one done removes it; `due_before` and `assigned` filters; cursor paging;
//! every org role reads it, another org is 404. Needs `DATABASE_URL`.

mod common;
mod participant_support;

use asd_contract::api_v1::Role;
use asd_contract::emr::FollowUpPage;
use axum::body::Body;
use http::{Method, Request, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn url(org: Uuid, q: &str) -> String {
    format!("/v1/orgs/{org}/follow-ups{q}")
}

async fn encounter(c: &Ctx, p: Uuid, bearer: &str, extra: Value) -> Value {
    let mut body = json!({"kind": "follow_up_visit", "occurred_at": "2026-10-20T08:00:00Z"});
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let (s, made) = c
        .call(
            Method::POST,
            &format!("/v1/participants/{p}/encounters"),
            Some(body),
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    made
}

async fn page(c: &Ctx, q: &str, bearer: &str) -> FollowUpPage {
    let (s, b) = c.call(Method::GET, &url(c.org, q), None, bearer).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    serde_json::from_value(b).unwrap()
}

#[tokio::test]
async fn open_follow_ups_sorted_by_due_and_done_ones_disappear() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "child-bbbbbbbbbbbbbbbbbbbb").await;
    let (_, admin) = c.token(Role::Admin).await;
    let (clin_id, clin) = c.token(Role::Clinician).await;

    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    let late = encounter(
        &c,
        p,
        &admin,
        json!({"follow_up_due_at": "2027-01-15T00:00:00Z"}),
    )
    .await;
    let due = encounter(
        &c,
        p,
        &admin,
        json!({"follow_up_due_at": yesterday, "clinician_id": clin_id.to_string(),
               "reason": "Check hearing"}),
    )
    .await;
    // No follow-up at all: never listed.
    encounter(&c, p, &admin, json!({})).await;

    let all = page(&c, "", &admin).await;
    assert_eq!(all.total, Some(2));
    let ids: Vec<&str> = all.items.iter().map(|f| f.encounter_id.as_str()).collect();
    assert_eq!(
        ids,
        [due["id"].as_str().unwrap(), late["id"].as_str().unwrap()]
    );
    let first = &all.items[0];
    assert_eq!(first.participant_token, "child-bbbbbbbbbbbbbbbbbbbb");
    assert_eq!(first.reason.as_deref(), Some("Check hearing"));
    assert_eq!(first.clinician_id, Some(clin_id.to_string()));

    // Filters.
    let before = page(&c, "?due_before=2026-12-31", &admin).await;
    assert_eq!(before.items.len(), 1);
    assert_eq!(before.items[0].encounter_id, due["id"].as_str().unwrap());
    let mine = page(&c, "?assigned=me", &clin).await;
    assert_eq!(mine.items.len(), 1);
    let none = page(&c, "?assigned=none", &admin).await;
    assert_eq!(none.items.len(), 1);
    assert_eq!(none.items[0].encounter_id, late["id"].as_str().unwrap());
    let by_id = page(&c, &format!("?assigned={clin_id}"), &admin).await;
    assert_eq!(by_id.items.len(), 1);
    let (s, _) = c
        .call(Method::GET, &url(c.org, "?due_before=soon"), None, &admin)
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // Mark it done: gone from the list.
    let id = due["id"].as_str().unwrap();
    let req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/v1/encounters/{id}"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {admin}"))
        .header("if-match", "\"1\"")
        .body(Body::from(
            json!({"follow_up_done_at": chrono::Utc::now().to_rfc3339()}).to_string(),
        ))
        .unwrap();
    let (s, _, b) = common::send(c.app.clone(), req).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let after = page(&c, "", &admin).await;
    assert_eq!(after.items.len(), 1);
    assert_eq!(after.items[0].encounter_id, late["id"].as_str().unwrap());
    c.db.cleanup().await;
}

#[tokio::test]
async fn paging_roles_and_scoping() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "child-cccccccccccccccccccc").await;
    let (_, admin) = c.token(Role::Admin).await;
    for day in 1..=5 {
        encounter(
            &c,
            p,
            &admin,
            json!({"follow_up_due_at": format!("2026-12-0{day}T00:00:00Z")}),
        )
        .await;
    }
    let mut seen = Vec::new();
    let mut q = "?limit=2".to_owned();
    loop {
        let pg = page(&c, &q, &admin).await;
        seen.extend(pg.items.iter().map(|f| f.follow_up_due_at.clone()));
        match pg.next_cursor {
            Some(cur) => q = format!("?limit=2&cursor={cur}"),
            None => break,
        }
    }
    let mut sorted = seen.clone();
    sorted.sort();
    assert_eq!(seen, sorted);
    assert_eq!(seen.len(), 5);

    for role in [
        Role::Owner,
        Role::Clinician,
        Role::Researcher,
        Role::Reviewer,
        Role::Viewer,
        Role::Superadmin,
    ] {
        let (_, t) = c.token(role).await;
        let (s, _) = c.call(Method::GET, &url(c.org, ""), None, &t).await;
        assert_eq!(s, StatusCode::OK, "{role:?}");
    }
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, b_admin) = c.token_in(Some(other), Role::Admin).await;
    let (s, _) = c.call(Method::GET, &url(c.org, ""), None, &b_admin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    // Clinic B's own list does not show clinic A's follow-ups.
    let (s, b) = c.call(Method::GET, &url(other, ""), None, &b_admin).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["total"], 0);
    c.db.cleanup().await;
}

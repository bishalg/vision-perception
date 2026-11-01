//! owner: WP-C.14 follow-up
//!
//! Backend gaps the console found: timeline filters, sessions by participant, the console
//! user's care summary and the sandbox's raw session payload. Needs `DATABASE_URL`; skipped
//! otherwise.

mod common;
mod participant_support;

use asd_contract::api_v1::{Role, SessionPage, SessionRaw};
use asd_contract::care::CareSummary;
use asd_contract::clinical::{TimelineEntryKind, TimelinePage};
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::json;
use uuid::Uuid;

async fn care(c: &Ctx, p: Uuid, at: &str, kind: &str, payload: serde_json::Value) {
    sqlx::query(
        "INSERT INTO care_events (org_id, participant_id, kind, occurred_at, payload)
         VALUES ($1, $2, $3, $4::timestamptz, $5)",
    )
    .bind(c.org)
    .bind(p)
    .bind(kind)
    .bind(at)
    .bind(payload)
    .execute(&c.db.pool)
    .await
    .unwrap();
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
async fn timeline_filters_by_kind_and_range_and_paginates_inside_them() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    for (i, day) in ["01", "02", "03", "04"].iter().enumerate() {
        c.session(
            c.org,
            p,
            &format!("S-{i}"),
            &format!("2026-10-{day}T10:00:00Z"),
        )
        .await;
        care(
            &c,
            p,
            &format!("2026-10-{day}T11:00:00Z"),
            "diary",
            json!({}),
        )
        .await;
    }
    let (_, viewer) = c.token(Role::Viewer).await;

    let all = timeline(&c, p, "", &viewer).await;
    assert_eq!(all.items.len(), 8);

    for q in [
        "?kind=session",
        "?kind=session&kind=session",
        "?kind=session,session",
    ] {
        let page = timeline(&c, p, q, &viewer).await;
        assert_eq!(page.items.len(), 4, "{q}");
        assert!(page
            .items
            .iter()
            .all(|e| e.kind == TimelineEntryKind::Session));
    }
    for q in ["?kind=session&kind=care_event", "?kind=session,care_event"] {
        assert_eq!(timeline(&c, p, q, &viewer).await.items.len(), 8, "{q}");
    }
    assert!(timeline(&c, p, "?kind=note", &viewer)
        .await
        .items
        .is_empty());

    // Dates are inclusive: a bare `to` day covers the whole day.
    let page = timeline(&c, p, "?from=2026-10-02&to=2026-10-03", &viewer).await;
    assert_eq!(page.items.len(), 4);
    let page = timeline(
        &c,
        p,
        "?kind=session&from=2026-10-02T10:00:00Z&to=2026-10-03T10:00:00Z",
        &viewer,
    )
    .await;
    let ids: Vec<_> = page.items.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["S-2", "S-1"]);

    // The cursor continues inside the filtered stream.
    let first = timeline(&c, p, "?kind=session&limit=3", &viewer).await;
    assert_eq!(first.items.len(), 3);
    let cur = first.next_cursor.expect("more sessions");
    let second = timeline(
        &c,
        p,
        &format!("?kind=session&limit=3&cursor={cur}"),
        &viewer,
    )
    .await;
    let ids: Vec<_> = second.items.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["S-0"]);
    assert!(second.next_cursor.is_none());

    for q in ["?kind=bogus", "?from=yesterday", "?to=2026-13-40"] {
        let (s, body) = c
            .call(
                Method::GET,
                &format!("/v1/participants/{p}/timeline{q}"),
                None,
                &viewer,
            )
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{q}: {body}");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn sessions_filter_by_participant() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    let q = c.participant(c.org, "P-2").await;
    c.session(c.org, p, "S-a", "2026-10-01T10:00:00Z").await;
    c.session(c.org, p, "S-b", "2026-10-02T10:00:00Z").await;
    c.session(c.org, q, "S-c", "2026-10-03T10:00:00Z").await;
    let (_, viewer) = c.token(Role::Viewer).await;
    let list = |query: String| {
        let (c, viewer) = (&c, &viewer);
        async move {
            c.call(
                Method::GET,
                &format!("/v1/orgs/{}/sessions{query}", c.org),
                None,
                viewer,
            )
            .await
        }
    };

    let (s, body) = list(format!("?participant_id={p}")).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let page: SessionPage = serde_json::from_value(body).unwrap();
    let ids: Vec<_> = page.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["S-b", "S-a"]);
    assert_eq!(page.total, Some(2));

    // With another filter and the cursor.
    let (_, body) = list(format!("?participant_id={p}&limit=1&flag=red")).await;
    let page: SessionPage = serde_json::from_value(body).unwrap();
    assert_eq!(page.items.len(), 1);
    let cur = page.next_cursor.unwrap();
    let (_, body) = list(format!("?participant_id={p}&limit=1&cursor={cur}")).await;
    let page: SessionPage = serde_json::from_value(body).unwrap();
    assert_eq!(page.items[0].id, "S-a");
    assert!(page.next_cursor.is_none());
    let (_, body) = list(format!("?participant_id={p}&flag=green")).await;
    let page: SessionPage = serde_json::from_value(body).unwrap();
    assert!(page.items.is_empty());

    // A participant of another org matches nothing; garbage is a 400.
    let org2 = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let foreign = c.participant(org2, "P-x").await;
    let (_, body) = list(format!("?participant_id={foreign}")).await;
    let page: SessionPage = serde_json::from_value(body).unwrap();
    assert!(page.items.is_empty());
    let (s, _) = list("?participant_id=not-a-uuid".into()).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    c.db.cleanup().await;
}

#[tokio::test]
async fn care_summary_for_console_users_is_clinical_and_audited() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    care(
        &c,
        p,
        "2026-10-01T10:00:00Z",
        "task",
        json!({"task_id": "session-1"}),
    )
    .await;
    care(
        &c,
        p,
        "2026-10-02T10:00:00Z",
        "diary",
        json!({"mood": "happy"}),
    )
    .await;
    let url = format!("/v1/participants/{p}/summary");

    let (clin_id, clinician) = c.token(Role::Clinician).await;
    let (s, body) = c.call(Method::GET, &url, None, &clinician).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let sum: CareSummary = serde_json::from_value(body).unwrap();
    assert_eq!(sum.participant_token, "P-1");
    assert_eq!(sum.total_events, 2);
    assert_eq!(sum.session_count, 1);
    assert_eq!(sum.recent_moods, ["happy"]);
    let rows = c.audit_rows("participant.care_summary_read").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["actor_id"], clin_id.to_string());
    assert_eq!(rows[0]["target_id"], p.to_string());

    let (_, owner) = c.token(Role::Owner).await;
    assert_eq!(
        c.call(Method::GET, &url, None, &owner).await.0,
        StatusCode::OK
    );

    // Roles without clinical content, and the vendor, are refused (RBAC-3); no audit row.
    for role in [
        Role::Admin,
        Role::Researcher,
        Role::Reviewer,
        Role::Viewer,
        Role::Superadmin,
    ] {
        let (_, t) = c.token(role).await;
        assert_eq!(
            c.call(Method::GET, &url, None, &t).await.0,
            StatusCode::FORBIDDEN,
            "{role:?}"
        );
    }
    assert_eq!(c.audit_rows("participant.care_summary_read").await.len(), 2);

    // Another org's clinician, an unknown id and a non-UUID id are 404; no credentials 401.
    let org2 = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, foreign) = c.token_in(Some(org2), Role::Clinician).await;
    assert_eq!(
        c.call(Method::GET, &url, None, &foreign).await.0,
        StatusCode::NOT_FOUND
    );
    for u in [
        format!("/v1/participants/{}/summary", Uuid::new_v4()),
        "/v1/participants/P-1/summary".to_owned(),
    ] {
        assert_eq!(
            c.call(Method::GET, &u, None, &clinician).await.0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        c.call(Method::GET, &url, None, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn raw_payload_is_de_identified_audited_and_role_limited() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    c.session(c.org, p, "S-1", "2026-10-01T10:00:00Z").await;
    sqlx::query(
        "UPDATE sessions SET schema_version = '1.8', frames_blob_key = 'k',
                summary = $2 WHERE id = $1",
    )
    .bind("S-1")
    .bind(json!({
        "sessionId": "S-1", "participantId": "P-1", "schemaVersion": "v1.8",
        "notes": "child is called Asha", "perVideo": []
    }))
    .execute(&c.db.pool)
    .await
    .unwrap();
    let url = "/v1/sessions/S-1/raw";

    let (uid, researcher) = c.token(Role::Researcher).await;
    let (s, body) = c.call(Method::GET, url, None, &researcher).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let raw: SessionRaw = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(raw.session_id, "S-1");
    assert_eq!(raw.schema_version, "1.8");
    assert_eq!(raw.participant_token, "P-1");
    assert!(raw.frames_available && !raw.frames_purged);
    assert_eq!(raw.raw_payload["participantId"], "P-1");
    assert!(raw.raw_payload.get("notes").is_none());
    assert!(!body.to_string().contains("Asha"));
    // The participant's row id never appears.
    assert!(!body.to_string().contains(&p.to_string()));
    let rows = c.audit_rows("session.raw_read").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["actor_id"], uid.to_string());
    assert_eq!(rows[0]["target_id"], "S-1");
    assert!(!c.audit_dump().await.contains("Asha"));

    for role in [Role::Owner, Role::Admin, Role::Clinician, Role::Superadmin] {
        let (_, t) = c.token(role).await;
        assert_eq!(
            c.call(Method::GET, url, None, &t).await.0,
            StatusCode::OK,
            "{role:?}"
        );
    }
    for role in [Role::Reviewer, Role::Viewer] {
        let (_, t) = c.token(role).await;
        assert_eq!(
            c.call(Method::GET, url, None, &t).await.0,
            StatusCode::FORBIDDEN,
            "{role:?}"
        );
    }
    let org2 = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, foreign) = c.token_in(Some(org2), Role::Owner).await;
    assert_eq!(
        c.call(Method::GET, url, None, &foreign).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.call(Method::GET, "/v1/sessions/nope/raw", None, &researcher)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(c.audit_rows("session.raw_read").await.len(), 5);
    c.db.cleanup().await;
}

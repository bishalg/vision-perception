//! owner: WP-B.19
//!
//! Encounters (REC-3, REC-4, REC-5; acceptance 2 and 3): the summary needs
//! `ClinicalNoteWrite` to write and `ClinicalContentRead` to read, is ciphertext at rest
//! bound to its row (AAD), and its reads are audited; session links; the closed-encounter
//! rules; `If-Match` / `412 stale-version`; the timeline headers. Needs `DATABASE_URL`.

mod common;
mod participant_support;

use asd_contract::api_v1::Role;
use asd_contract::clinical::{TimelineEntryKind, TimelinePage};
use axum::body::Body;
use http::{Method, Request, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

const SUMMARY: &str = "SECRET-ENCOUNTER-SUMMARY: calm, good joint attention";
const REASON: &str = "Routine screening visit";

fn encounters_url(p: impl std::fmt::Display) -> String {
    format!("/v1/participants/{p}/encounters")
}

async fn patch(
    c: &Ctx,
    id: &str,
    version: Option<&str>,
    body: Value,
    bearer: &str,
) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/v1/encounters/{id}"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {bearer}"));
    if let Some(v) = version {
        b = b.header("if-match", v);
    }
    let (s, _, body) = common::send(
        c.app.clone(),
        b.body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap(),
    )
    .await;
    (s, body)
}

async fn setup(c: &Ctx) -> Uuid {
    let p = c.participant(c.org, "child-aaaaaaaaaaaaaaaaaaaa").await;
    c.session(c.org, p, "SESS-ENC-1", "2026-11-02T09:00:00Z")
        .await;
    p
}

#[tokio::test]
async fn admin_cannot_write_a_summary_clinician_can_and_it_is_encrypted() {
    let Some(c) = Ctx::new().await else { return };
    let p = setup(&c).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (clin_id, clin) = c.token(Role::Clinician).await;

    // Acceptance 2: front desk adds the visit; a summary is refused.
    let (s, b) = c
        .call(
            Method::POST,
            &encounters_url(p),
            Some(
                json!({"kind": "screening_visit", "occurred_at": "2026-11-02T08:30:00Z",
                        "reason": REASON, "summary": SUMMARY}),
            ),
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "encounter-summary-forbidden");
    let (s, made) = c
        .call(
            Method::POST,
            &encounters_url(p),
            Some(
                json!({"kind": "screening_visit", "occurred_at": "2026-11-02T08:30:00Z",
                        "reason": REASON, "clinician_id": clin_id.to_string()}),
            ),
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    assert_eq!(made["has_summary"], false);
    assert_eq!(made["status"], "open");
    assert_eq!(made["row_version"], 1);
    assert_eq!(made["reason"], REASON);
    assert_eq!(made["clinician_id"], clin_id.to_string());
    let id = made["id"].as_str().unwrap().to_owned();

    // The clinician adds the summary and links the session.
    let (s, upd) = patch(&c, &id, Some("\"1\""), json!({"summary": SUMMARY}), &clin).await;
    assert_eq!(s, StatusCode::OK, "{upd}");
    assert_eq!(upd["summary"], SUMMARY);
    assert_eq!(upd["has_summary"], true);
    assert_eq!(upd["row_version"], 2);
    let (s, linked) = c
        .call(
            Method::POST,
            &format!("/v1/encounters/{id}/sessions"),
            Some(json!({"session_id": "SESS-ENC-1"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{linked}");
    assert_eq!(linked["session_ids"], json!(["SESS-ENC-1"]));
    // The link answer shows the clinician the summary: that is a read (the PATCH echo of
    // the text the clinician just wrote is not).
    assert_eq!(linked["summary"], SUMMARY);
    assert_eq!(c.audit_rows("encounter.read").await.len(), 1);

    // At rest: ciphertext, never the text.
    let ct: Vec<u8> = sqlx::query_scalar("SELECT summary_ct FROM encounters WHERE id = $1::uuid")
        .bind(&id)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&ct).contains("SECRET-ENCOUNTER"));
    assert!(!c.audit_dump().await.contains("SECRET-ENCOUNTER"));

    // Superadmin and admin see the header and has_summary, never the text; no read audit.
    let (_, root) = c.token(Role::Superadmin).await;
    for t in [&root, &admin] {
        let (s, one) = c
            .call(Method::GET, &format!("/v1/encounters/{id}"), None, t)
            .await;
        assert_eq!(s, StatusCode::OK, "{one}");
        assert_eq!(one["has_summary"], true);
        assert!(one.get("summary").is_none(), "{one}");
    }
    assert_eq!(c.audit_rows("encounter.read").await.len(), 1);

    // Owner reads the summary: audited first.
    let (_, owner) = c.token(Role::Owner).await;
    let (s, one) = c
        .call(Method::GET, &format!("/v1/encounters/{id}"), None, &owner)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(one["summary"], SUMMARY);
    let reads = c.audit_rows("encounter.read").await;
    assert_eq!(reads.len(), 2);
    assert_eq!(reads[1]["target_id"], id);

    // The list: summaries for the owner (one audit row per page with ids), none for viewer.
    let (s, page) = c.call(Method::GET, &encounters_url(p), None, &owner).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["summary"], SUMMARY);
    let reads = c.audit_rows("encounter.read").await;
    assert_eq!(reads.len(), 3);
    assert_eq!(reads[2]["after"]["encounter_ids"], json!([id]));
    let (_, viewer) = c.token(Role::Viewer).await;
    let (s, page) = c.call(Method::GET, &encounters_url(p), None, &viewer).await;
    assert_eq!(s, StatusCode::OK);
    assert!(page["items"][0].get("summary").is_none());
    assert_eq!(page["items"][0]["has_summary"], true);
    assert_eq!(c.audit_rows("encounter.read").await.len(), 3);

    // Timeline: the encounter header only; the session carries encounter_id.
    let (s, tl) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{p}/timeline"),
            None,
            &viewer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{tl}");
    let tl: TimelinePage = serde_json::from_value(tl).unwrap();
    let enc = tl
        .items
        .iter()
        .find(|e| e.kind == TimelineEntryKind::Encounter)
        .unwrap();
    assert_eq!(enc.id, id);
    assert_eq!(enc.summary["encounter_kind"], "screening_visit");
    assert_eq!(enc.summary["status"], "open");
    assert_eq!(enc.actor_id.as_deref(), Some(clin_id.to_string().as_str()));
    let ses = tl
        .items
        .iter()
        .find(|e| e.kind == TimelineEntryKind::Session)
        .unwrap();
    assert_eq!(ses.summary["encounter_id"], id);
    assert!(!serde_json::to_string(&tl).unwrap().contains("SECRET"));

    // Audits of writes carry has_summary, never text.
    let upd_audit = c.audit_rows("encounter.update").await;
    assert_eq!(upd_audit[0]["after"]["has_summary"], true);
    assert_eq!(upd_audit[0]["before"]["has_summary"], false);
    assert_eq!(c.audit_rows("encounter.session_link").await.len(), 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn clinician_creates_with_summary_and_sessions_in_one_call() {
    let Some(c) = Ctx::new().await else { return };
    let p = setup(&c).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (s, made) = c
        .call(
            Method::POST,
            &encounters_url(p),
            Some(
                json!({"kind": "follow_up_visit", "occurred_at": "2026-11-02T10:00:00Z",
                        "summary": SUMMARY, "session_ids": ["SESS-ENC-1"],
                        "follow_up_due_at": "2026-12-01T00:00:00Z"}),
            ),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    assert_eq!(made["summary"], SUMMARY);
    assert_eq!(made["session_ids"], json!(["SESS-ENC-1"]));
    // The writer's own echo is not a read.
    assert!(c.audit_rows("encounter.read").await.is_empty());
    let create = c.audit_rows("encounter.create").await;
    assert_eq!(create[0]["after"]["has_summary"], true);
    assert_eq!(create[0]["after"]["session_ids"], json!(["SESS-ENC-1"]));

    // A session already linked elsewhere is a conflict; an unknown one 400.
    let (s, b) = c
        .call(
            Method::POST,
            &encounters_url(p),
            Some(
                json!({"kind": "other", "occurred_at": "2026-11-02T11:00:00Z",
                        "session_ids": ["SESS-ENC-1"]}),
            ),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    let (s, b) = c
        .call(
            Method::POST,
            &encounters_url(p),
            Some(
                json!({"kind": "other", "occurred_at": "2026-11-02T11:00:00Z",
                        "session_ids": ["SESS-NOPE"]}),
            ),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    // Nothing was left behind by the refused creates.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM encounters")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(n, 1);

    // Unlink, then unlink again: 404.
    let id = made["id"].as_str().unwrap();
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/encounters/{id}/sessions/SESS-ENC-1"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/encounters/{id}/sessions/SESS-ENC-1"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn stale_version_and_closed_rules() {
    let Some(c) = Ctx::new().await else { return };
    let p = setup(&c).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, made) = c
        .call(
            Method::POST,
            &encounters_url(p),
            Some(json!({"kind": "phone_call", "occurred_at": "2026-11-02T08:00:00Z"})),
            &admin,
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();

    // Acceptance 3: two tabs edit; the second save gets 412. No If-Match: 400.
    let (s, _) = patch(&c, &id, Some("\"1\""), json!({"reason": "Tab one"}), &admin).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = patch(&c, &id, Some("\"1\""), json!({"reason": "Tab two"}), &admin).await;
    assert_eq!(s, StatusCode::PRECONDITION_FAILED, "{b}");
    assert_eq!(slug_of(&b), "stale-version");
    let (s, b) = patch(&c, &id, None, json!({"reason": "x"}), &admin).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");

    // Close it.
    let (s, b) = patch(
        &c,
        &id,
        Some("\"2\""),
        json!({"status": "closed", "follow_up_due_at": "2026-11-20T00:00:00Z"}),
        &admin,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["status"], "closed");
    // Closed: other fields are refused, follow_up_done_at is accepted.
    let (s, b) = patch(
        &c,
        &id,
        Some("\"3\""),
        json!({"reason": "late edit"}),
        &admin,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug_of(&b), "encounter-closed");
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/encounters/{id}/sessions"),
            Some(json!({"session_id": "SESS-ENC-1"})),
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    let (s, b) = patch(
        &c,
        &id,
        Some("\"3\""),
        json!({"follow_up_done_at": "2026-11-03T00:00:00Z"}),
        &admin,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["follow_up_done_at"], "2026-11-03T00:00:00.000Z");
    // Reopen: admin may not, clinician may.
    let (s, b) = patch(&c, &id, Some("\"4\""), json!({"status": "open"}), &admin).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug_of(&b), "encounter-closed");
    let (s, b) = patch(
        &c,
        &id,
        Some("W/\"4\""),
        json!({"status": "open", "reason": ""}),
        &clin,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["status"], "open");
    assert_eq!(b["reason"], Value::Null, "empty string clears");
    c.db.cleanup().await;
}

#[tokio::test]
async fn roles_and_scoping() {
    let Some(c) = Ctx::new().await else { return };
    let p = setup(&c).await;
    let body = json!({"kind": "home_visit", "occurred_at": "2026-11-02T08:00:00Z"});
    for role in [Role::Viewer, Role::Researcher, Role::Reviewer] {
        let (_, t) = c.token(role).await;
        let (s, _) = c
            .call(Method::POST, &encounters_url(p), Some(body.clone()), &t)
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?}");
        let (s, _) = c.call(Method::GET, &encounters_url(p), None, &t).await;
        assert_eq!(s, StatusCode::OK, "{role:?} may read headers");
    }
    // Superadmin: header-only writes, like admin (REC-8); a summary is refused.
    let (_, root) = c.token(Role::Superadmin).await;
    let (s, created) = c
        .call(Method::POST, &encounters_url(p), Some(body.clone()), &root)
        .await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap().to_owned();
    let mut with_summary = body.clone();
    with_summary["summary"] = json!("text");
    let (s, b) = c
        .call(Method::POST, &encounters_url(p), Some(with_summary), &root)
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "encounter-summary-forbidden");

    // Another org's clinician: 404 on the participant and the encounter.
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, b_clin) = c.token_in(Some(other), Role::Clinician).await;
    let (s, _) = c
        .call(
            Method::POST,
            &encounters_url(p),
            Some(body.clone()),
            &b_clin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c
        .call(Method::GET, &format!("/v1/encounters/{id}"), None, &b_clin)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c
        .call(Method::GET, "/v1/encounters/not-a-uuid", None, &root)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Validation and the withdrawn rule.
    let (_, admin) = c.token(Role::Admin).await;
    for bad in [
        json!({"kind": "teleport", "occurred_at": "2026-11-02T08:00:00Z"}),
        json!({"kind": "other", "occurred_at": "today"}),
        json!({"kind": "other", "occurred_at": "2026-11-02T08:00:00Z", "reason": "r".repeat(201)}),
        json!({"kind": "other", "occurred_at": "2026-11-02T08:00:00Z",
               "clinician_id": Uuid::new_v4().to_string()}),
    ] {
        let (s, b) = c
            .call(Method::POST, &encounters_url(p), Some(bad.clone()), &admin)
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}: {b}");
    }
    sqlx::query("UPDATE participants SET withdrawn_at = now() WHERE id = $1")
        .bind(p)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, b) = c
        .call(Method::POST, &encounters_url(p), Some(body), &admin)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug_of(&b), "participant-withdrawn");
    c.db.cleanup().await;
}

#[tokio::test]
async fn summary_ciphertext_copied_to_another_row_fails_with_500_and_no_plaintext() {
    let Some(c) = Ctx::new().await else { return };
    let p = setup(&c).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let mut ids = Vec::new();
    for text in [SUMMARY, "second summary"] {
        let (s, made) = c
            .call(
                Method::POST,
                &encounters_url(p),
                Some(
                    json!({"kind": "other", "occurred_at": "2026-11-02T08:00:00Z",
                            "summary": text}),
                ),
                &clin,
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{made}");
        ids.push(made["id"].as_str().unwrap().to_owned());
    }
    sqlx::query(
        "UPDATE encounters SET (summary_ct, summary_key_id) =
             (SELECT summary_ct, summary_key_id FROM encounters WHERE id = $1::uuid)
         WHERE id = $2::uuid",
    )
    .bind(&ids[0])
    .bind(&ids[1])
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (s, b) = c
        .call(
            Method::GET,
            &format!("/v1/encounters/{}", ids[1]),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR, "{b}");
    assert!(!b.to_string().contains("SECRET"), "{b}");
    // The untouched row still opens.
    let (s, b) = c
        .call(
            Method::GET,
            &format!("/v1/encounters/{}", ids[0]),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["summary"], SUMMARY);
    c.db.cleanup().await;
}

#[tokio::test]
async fn list_pages_newest_first() {
    let Some(c) = Ctx::new().await else { return };
    let p = setup(&c).await;
    let (_, admin) = c.token(Role::Admin).await;
    for day in 1..=5 {
        let (s, _) = c
            .call(
                Method::POST,
                &encounters_url(p),
                Some(json!({"kind": "other",
                            "occurred_at": format!("2026-10-0{day}T08:00:00Z")})),
                &admin,
            )
            .await;
        assert_eq!(s, StatusCode::CREATED);
    }
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let url = match &cursor {
            Some(cur) => format!("{}?limit=2&cursor={cur}", encounters_url(p)),
            None => format!("{}?limit=2", encounters_url(p)),
        };
        let (s, page) = c.call(Method::GET, &url, None, &admin).await;
        assert_eq!(s, StatusCode::OK, "{page}");
        assert_eq!(page["total"], 5);
        for item in page["items"].as_array().unwrap() {
            seen.push(item["occurred_at"].as_str().unwrap().to_owned());
        }
        match page["next_cursor"].as_str() {
            Some(n) => cursor = Some(n.to_owned()),
            None => break,
        }
    }
    let mut sorted = seen.clone();
    sorted.sort();
    sorted.reverse();
    assert_eq!(seen, sorted);
    assert_eq!(seen.len(), 5);
    c.db.cleanup().await;
}

#[tokio::test]
async fn omitted_clinician_defaults_to_a_clinician_caller_only() {
    let Some(c) = Ctx::new().await else { return };
    let p = setup(&c).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (clin_id, clin) = c.token(Role::Clinician).await;
    let body = json!({"kind": "follow_up_visit", "occurred_at": "2026-11-02T10:00:00Z",
                      "follow_up_due_at": "2026-12-01T00:00:00Z"});

    // A clinician who leaves the field out is the visit's clinician.
    let (s, mine) = c
        .call(Method::POST, &encounters_url(p), Some(body.clone()), &clin)
        .await;
    assert_eq!(s, StatusCode::CREATED, "{mine}");
    assert_eq!(mine["clinician_id"], clin_id.to_string());
    let mine_id: Uuid = mine["id"].as_str().unwrap().parse().unwrap();
    let stored: Option<Uuid> =
        sqlx::query_scalar("SELECT clinician_id FROM encounters WHERE id = $1")
            .bind(mine_id)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(stored, Some(clin_id));

    // An admin's visit stays unassigned.
    let (s, theirs) = c
        .call(Method::POST, &encounters_url(p), Some(body), &admin)
        .await;
    assert_eq!(s, StatusCode::CREATED, "{theirs}");
    assert!(theirs["clinician_id"].is_null(), "{theirs}");
    let theirs_id: Uuid = theirs["id"].as_str().unwrap().parse().unwrap();
    let stored: Option<Uuid> =
        sqlx::query_scalar("SELECT clinician_id FROM encounters WHERE id = $1")
            .bind(theirs_id)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(stored, None);

    // `assigned=me` lists the clinician's own follow-up and not the admin's.
    let (s, b) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/follow-ups?assigned=me", c.org),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let ids: Vec<&str> = b["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["encounter_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [mine_id.to_string()]);
    c.db.cleanup().await;
}

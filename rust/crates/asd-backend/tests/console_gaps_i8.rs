//! owner: console gaps (C.3, I.8)
//!
//! `GET /orgs/{id}/users/{user_id}`, audit list filters, `GET /orgs/{id}/integrations`
//! paging, `GET /orgs/{id}/export-destinations` (exporter picker, no config) and
//! `DELETE /integrations/{id}` (archive, `409` while deliveries are in flight). Needs
//! `DATABASE_URL`; skipped otherwise.

mod common;
mod delivery_support;
mod export_support;
mod participant_support;

use asd_contract::api_v1::Role;
use http::{Method, StatusCode};
use serde_json::{json, Value};

fn hook(name: &str) -> Value {
    json!({"kind": "webhook", "name": name, "config": {"url": "https://his.example.org/hook?k=zz-secret"}})
}

#[tokio::test]
async fn user_read_follows_the_users_list_rules() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, owner) = c.token(Role::Owner).await;
    let (uid, _) = c.token(Role::Clinician).await;
    let uri = format!("/v1/orgs/{}/users/{uid}", c.org);
    let (s, u) = c.call(Method::GET, &uri, None, &owner).await;
    assert_eq!(s, StatusCode::OK, "{u}");
    assert_eq!(u["id"], uid.to_string());
    assert_eq!(u["role"], "clinician");
    assert_eq!(u["org_id"], c.org.to_string());
    assert!(u.get("pw_hash").is_none());
    let (_, admin) = c.token(Role::Admin).await;
    assert_eq!(
        c.call(Method::GET, &uri, None, &admin).await.0,
        StatusCode::OK
    );
    for role in [
        Role::Clinician,
        Role::Researcher,
        Role::Reviewer,
        Role::Viewer,
    ] {
        let (_, t) = c.token(role).await;
        assert_eq!(
            c.call(Method::GET, &uri, None, &t).await.0,
            StatusCode::FORBIDDEN,
            "{role:?}"
        );
    }
    // Unknown, malformed, and another org's user are 404.
    let missing = format!("/v1/orgs/{}/users/{}", c.org, uuid::Uuid::new_v4());
    assert_eq!(
        c.call(Method::GET, &missing, None, &owner).await.0,
        StatusCode::NOT_FOUND
    );
    let bad = format!("/v1/orgs/{}/users/nope", c.org);
    assert_eq!(
        c.call(Method::GET, &bad, None, &owner).await.0,
        StatusCode::NOT_FOUND
    );
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (oid, _) = c.token_in(Some(other), Role::Viewer).await;
    let cross = format!("/v1/orgs/{}/users/{oid}", c.org);
    assert_eq!(
        c.call(Method::GET, &cross, None, &owner).await.0,
        StatusCode::NOT_FOUND
    );
    // Another org's id in the path is refused for this owner.
    let foreign = format!("/v1/orgs/{other}/users/{oid}");
    assert_ne!(
        c.call(Method::GET, &foreign, None, &owner).await.0,
        StatusCode::OK
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn audit_list_filters_combine_and_paginate() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (owner_id, owner) = c.token(Role::Owner).await;
    let (admin_id, admin) = c.token(Role::Admin).await;
    let a = c.destination(&owner, hook("A")).await;
    let b = c.destination(&admin, hook("B")).await;
    let a_id = a["id"].as_str().unwrap();
    let (s, _) = c
        .call(
            Method::PATCH,
            &format!("/v1/integrations/{a_id}"),
            Some(json!({"name": "A2"})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let base = format!("/v1/orgs/{}/audit", c.org);
    let get = |q: String| {
        let (c, owner, base) = (&c, owner.clone(), base.clone());
        async move {
            c.call(Method::GET, &format!("{base}?{q}"), None, &owner)
                .await
        }
    };
    let n = |v: &Value| v["items"].as_array().unwrap().len();

    let (s, all) = get(String::new()).await;
    assert_eq!(s, StatusCode::OK, "{all}");
    assert_eq!(n(&all), 3);

    // actor
    let (_, by_admin) = get(format!("actor_id={admin_id}")).await;
    assert_eq!(n(&by_admin), 1);
    assert_eq!(by_admin["items"][0]["target_id"], b["id"]);
    let (_, by_owner) = get(format!("actor_id={owner_id}")).await;
    assert_eq!(n(&by_owner), 2);
    // action: exact and prefix; "%" and "_" are literal
    let (_, exact) = get("action=integration.update".into()).await;
    assert_eq!(n(&exact), 1);
    let (_, prefix) = get("action=integration.".into()).await;
    assert_eq!(n(&prefix), 3);
    let (_, wild) = get("action=integration%25".into()).await;
    assert_eq!(n(&wild), 0);
    let (_, wild) = get("action=integration_".into()).await;
    assert_eq!(n(&wild), 0);
    // target type / id
    let (_, t) = get("target_type=integration".into()).await;
    assert_eq!(n(&t), 3);
    let (_, t) = get("target_type=user".into()).await;
    assert_eq!(n(&t), 0);
    let (_, t) = get(format!("target_type=integration&target_id={a_id}")).await;
    assert_eq!(n(&t), 2);
    // combined
    let (_, t) = get(format!("target_id={a_id}&action=integration.update")).await;
    assert_eq!(n(&t), 1);
    // time window: everything after "now" is empty, from the epoch is everything
    let (_, t) = get("from=2999-01-01T00:00:00Z".into()).await;
    assert_eq!(n(&t), 0);
    let (_, t) = get("from=2000-01-01T00:00:00Z&to=2999-01-01T00:00:00Z".into()).await;
    assert_eq!(n(&t), 3);
    let (_, t) = get("to=2000-01-01T00:00:00Z".into()).await;
    assert_eq!(n(&t), 0);
    // pagination keeps the filter
    let (_, p1) = get("action=integration.&limit=2".into()).await;
    assert_eq!(n(&p1), 2);
    let cur = p1["next_cursor"].as_str().expect("more");
    let (_, p2) = get(format!("action=integration.&limit=2&cursor={cur}")).await;
    assert_eq!(n(&p2), 1);
    assert!(p2["next_cursor"].is_null());
    // validation
    for q in [
        "from=yesterday",
        "to=2026-13-40",
        "from=2026-02-01T00:00:00Z&to=2026-01-01T00:00:00Z",
    ] {
        let (s, body) = get(q.into()).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{q}: {body}");
    }
    let (s, _) = get(format!("actor_id={}", "x".repeat(201))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // another org's rows never leak through a filter
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, other_owner) = c.token_in(Some(other), Role::Owner).await;
    let (s, t) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{other}/audit?action=integration."),
            None,
            &other_owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(n(&t), 0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn integrations_list_pages_with_cursor_and_limit() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, owner) = c.token(Role::Owner).await;
    for name in ["one", "two", "three"] {
        c.destination(&owner, hook(name)).await;
    }
    let list = format!("/v1/orgs/{}/integrations", c.org);
    let (s, p1) = c
        .call(Method::GET, &format!("{list}?limit=2"), None, &owner)
        .await;
    assert_eq!(s, StatusCode::OK, "{p1}");
    assert_eq!(p1["items"].as_array().unwrap().len(), 2);
    assert_eq!(p1["total"], 3);
    let cur = p1["next_cursor"].as_str().expect("more");
    let (_, p2) = c
        .call(
            Method::GET,
            &format!("{list}?limit=2&cursor={cur}"),
            None,
            &owner,
        )
        .await;
    assert_eq!(p2["items"].as_array().unwrap().len(), 1);
    assert!(p2["next_cursor"].is_null());
    let mut names: Vec<&str> = p1["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(p2["items"].as_array().unwrap())
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["one", "three", "two"]);
    for q in ["limit=0", "limit=101", "limit=x", "cursor=%21%21"] {
        let (s, _) = c
            .call(Method::GET, &format!("{list}?{q}"), None, &owner)
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{q}");
    }
    let (_, p) = c
        .call(Method::GET, &format!("{list}?limit=100"), None, &owner)
        .await;
    assert_eq!(p["items"].as_array().unwrap().len(), 3);
    c.db.cleanup().await;
}

#[tokio::test]
async fn exporters_pick_destinations_without_seeing_config() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, owner) = c.token(Role::Owner).await;
    let live = c.destination(&owner, hook("Live")).await;
    let off = c.destination(&owner, hook("Off")).await;
    let (s, _) = c
        .call(
            Method::PATCH,
            &format!("/v1/integrations/{}", off["id"].as_str().unwrap()),
            Some(json!({"status": "disabled"})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let uri = format!("/v1/orgs/{}/export-destinations", c.org);
    for role in [Role::Owner, Role::Admin, Role::Clinician, Role::Researcher] {
        let (_, t) = c.token(role).await;
        let (s, body) = c.call(Method::GET, &uri, None, &t).await;
        assert_eq!(s, StatusCode::OK, "{role:?}: {body}");
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1, "{role:?}: only active destinations");
        let item = items[0].as_object().unwrap();
        let mut keys: Vec<&str> = item.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["environment", "id", "kind", "name", "status"]);
        assert_eq!(item["id"], live["id"]);
        assert_eq!(item["status"], "active");
        assert!(!body.to_string().contains("zz-secret"));
    }
    for role in [Role::Reviewer, Role::Viewer] {
        let (_, t) = c.token(role).await;
        assert_eq!(
            c.call(Method::GET, &uri, None, &t).await.0,
            StatusCode::FORBIDDEN,
            "{role:?}"
        );
    }
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, other_clin) = c.token_in(Some(other), Role::Clinician).await;
    let (s, _) = c.call(Method::GET, &uri, None, &other_clin).await;
    assert_ne!(s, StatusCode::OK, "another org's clinician is refused");
    let (_, body) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{other}/export-destinations"),
            None,
            &other_clin,
        )
        .await;
    assert!(body["items"].as_array().unwrap().is_empty());
    c.db.cleanup().await;
}

#[tokio::test]
async fn delete_archives_audits_and_refuses_while_in_flight() {
    let Some(c) = delivery_support::ctx().await else {
        return;
    };
    let (_, owner) = c.token(Role::Owner).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, admin) = c.token(Role::Admin).await;
    let p = c.participant(c.org, "TOKEN-ALPHA").await;
    c.exportable_session(
        p,
        "TOKEN-ALPHA",
        "SESS-ONE",
        "2026-10-01T10:00:00Z",
        Some(true),
    )
    .await;
    let busy = c.destination(&owner, hook("Busy")).await;
    let idle = c.destination(&owner, hook("Idle")).await;
    let busy_id = busy["id"].as_str().unwrap();
    let idle_id = idle["id"].as_str().unwrap();

    // RBAC: only owner and admin.
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/integrations/{idle_id}"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, other_admin) = c.token_in(Some(other), Role::Admin).await;
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/integrations/{idle_id}"),
            None,
            &other_admin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // A pending delivery blocks the delete with 409.
    let mut body = export_support::body("csv", "identified", "clinical_care");
    body["destination_ids"] = json!([busy["id"]]);
    let (s, made) = c.request_export(&clin, body).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    let (s, problem) = c
        .call(
            Method::DELETE,
            &format!("/v1/integrations/{busy_id}"),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{problem}");
    assert_eq!(common::auth::slug(&problem), "conflict");
    assert!(c.audit_rows("integration.delete").await.is_empty());
    assert_eq!(
        c.call(
            Method::GET,
            &format!("/v1/integrations/{busy_id}"),
            None,
            &owner
        )
        .await
        .0,
        StatusCode::OK
    );

    // A finished delivery does not block; the destination disappears but its log stays.
    sqlx::query("UPDATE export_deliveries SET status = 'delivered', delivered_at = now() WHERE destination_id = $1::uuid")
        .bind(busy_id)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/integrations/{busy_id}"),
            None,
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let kept: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM export_deliveries WHERE destination_id = $1::uuid",
    )
    .bind(busy_id)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(kept, 1);
    let rows = c.audit_rows("integration.delete").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["target_id"], busy_id);
    assert_eq!(rows[0]["before"]["name"], "Busy");
    assert!(!c.audit_dump().await.contains("zz-secret"));

    // Gone from read, patch, secret, list, picker and new exports; second delete is 404.
    let one = format!("/v1/integrations/{busy_id}");
    assert_eq!(
        c.call(Method::GET, &one, None, &owner).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.call(
            Method::PATCH,
            &one,
            Some(json!({"status": "active"})),
            &owner
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.call(Method::DELETE, &one, None, &owner).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, page) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/integrations", c.org),
            None,
            &owner,
        )
        .await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["id"], idle["id"]);
    let (_, pick) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/export-destinations", c.org),
            None,
            &clin,
        )
        .await;
    assert_eq!(pick["items"].as_array().unwrap().len(), 1);
    let mut again = export_support::body("csv", "identified", "clinical_care");
    again["destination_ids"] = json!([busy["id"]]);
    let (s, _) = c.request_export(&clin, again).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // An idle destination deletes at once.
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/integrations/{idle_id}"),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(c.audit_rows("integration.delete").await.len(), 2);
    c.db.cleanup().await;
}

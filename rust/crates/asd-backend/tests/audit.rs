//! owner: WP-B.2
//!
//! AUTH-5: every mutating B.2 handler writes exactly one `audit_log` row in its own
//! transaction (none on failure), rows carry actor/target/ip/user agent and never a
//! secret, and `audit_log` is append-only (`0002_auth_grants.sql`: the application role
//! has no UPDATE/DELETE/TRUNCATE and a trigger refuses them for every role).
//! `GET /orgs/{id}/audit` lists one org's rows. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::app::AppState;
use asd_contract::api_v1::Role;
use axum::body::Body;
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
    org: Uuid,
    owner_id: Uuid,
    owner: String,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let owner_id = seed_user(&db.pool, Some(org), Role::Owner, "owner@example.org").await;
    let owner = access_token(&state, owner_id, Some(org), Role::Owner);
    Some(Ctx {
        db,
        state,
        app,
        org,
        owner_id,
        owner,
    })
}

fn with_client(mut req: Request<Body>) -> Request<Body> {
    req.headers_mut()
        .insert(http::header::USER_AGENT, "CurvConsole/1.0".parse().unwrap());
    from_ip(req, "192.0.2.10")
}

#[derive(Debug, sqlx::FromRow)]
struct Row {
    actor_type: String,
    actor_id: Option<String>,
    org_id: Option<Uuid>,
    action: String,
    target_type: String,
    target_id: Option<String>,
    before: Option<Value>,
    after: Option<Value>,
    ip: Option<String>,
    user_agent: Option<String>,
}

async fn last_row(c: &Ctx) -> Row {
    sqlx::query_as(
        "SELECT actor_type, actor_id, org_id, action, target_type, target_id, before, after, \
                host(ip) AS ip, user_agent \
         FROM audit_log ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&c.db.pool)
    .await
    .unwrap()
}

/// Runs `req`, asserts `status`, and asserts exactly one new audit row, returned.
async fn one_row(c: &Ctx, req: Request<Body>, status: StatusCode) -> (Row, Value) {
    let before = audit_total(&c.db.pool).await;
    let (got, _, body) = send(c.app.clone(), with_client(req)).await;
    assert_eq!(got, status, "{body}");
    assert_eq!(
        audit_total(&c.db.pool).await,
        before + 1,
        "exactly one audit row"
    );
    (last_row(c).await, body)
}

#[tokio::test]
async fn every_mutating_route_writes_exactly_one_row_with_context() {
    let Some(c) = setup().await else { return };
    let owner = Some(c.owner.as_str());
    let actor = c.owner_id.to_string();

    // PATCH /orgs/{id}
    let (row, _) = one_row(
        &c,
        json(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            &json!({"retention_months": 12}),
            owner,
        ),
        StatusCode::OK,
    )
    .await;
    assert_eq!(row.action, "org.update");
    assert_eq!(
        (row.actor_type.as_str(), row.actor_id.as_deref()),
        ("user", Some(actor.as_str()))
    );
    assert_eq!(row.org_id, Some(c.org));
    assert_eq!(row.target_type, "organization");
    assert_eq!(row.target_id, Some(c.org.to_string()));
    assert_eq!(row.before.as_ref().unwrap()["retention_months"], 24);
    assert_eq!(row.after.as_ref().unwrap()["retention_months"], 12);
    assert_eq!(row.ip.as_deref(), Some("192.0.2.10"));
    assert_eq!(row.user_agent.as_deref(), Some("CurvConsole/1.0"));

    // POST /orgs/{id}/users
    let (row, created) = one_row(
        &c,
        json(
            Method::POST,
            &format!("/v1/orgs/{}/users", c.org),
            &json!({"email": "new@example.org", "role": "viewer", "password": "a long enough password"}),
            owner,
        ),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(row.action, "user.create");
    assert_eq!(row.target_type, "user");
    let new_user = created["id"].as_str().unwrap().to_owned();
    assert_eq!(row.target_id.as_deref(), Some(new_user.as_str()));
    assert_eq!(row.after.as_ref().unwrap()["role"], "viewer");
    assert!(row.before.is_none());

    // PATCH /orgs/{id}/users/{user_id}
    let (row, _) = one_row(
        &c,
        json(
            Method::PATCH,
            &format!("/v1/orgs/{}/users/{new_user}", c.org),
            &json!({"role": "reviewer"}),
            owner,
        ),
        StatusCode::OK,
    )
    .await;
    assert_eq!(row.action, "user.update");
    assert_eq!(row.before.as_ref().unwrap()["role"], "viewer");
    assert_eq!(row.after.as_ref().unwrap()["role"], "reviewer");

    // POST /orgs/{id}/devices/enrol-code
    let (row, code) = one_row(
        &c,
        json(
            Method::POST,
            &format!("/v1/orgs/{}/devices/enrol-code", c.org),
            &json!({"max_uses": 2}),
            owner,
        ),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(row.action, "enrol_code.create");
    assert_eq!(row.target_type, "enrol_code");
    assert_eq!(row.target_id.as_deref(), code["id"].as_str());
    let code = code["code"].as_str().unwrap().to_owned();

    // POST /devices/register (device actor)
    let reg = json!({"enrol_code": code, "platform": "iOS", "app_version": "1.0"});
    let (row, device) = one_row(
        &c,
        json(Method::POST, "/v1/devices/register", &reg, None),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(row.action, "device.register");
    let device_id = device["device_id"].as_str().unwrap().to_owned();
    assert_eq!(row.actor_type, "device");
    assert_eq!(row.actor_id.as_deref(), Some(device_id.as_str()));
    assert_eq!(row.org_id, Some(c.org));
    assert_eq!(row.target_id.as_deref(), Some(device_id.as_str()));

    // Re-registration with device_id (rotation)
    let mut rereg = reg.clone();
    rereg["device_id"] = json!(device_id);
    let (row, _) = one_row(
        &c,
        json(Method::POST, "/v1/devices/register", &rereg, None),
        StatusCode::OK,
    )
    .await;
    assert_eq!(row.action, "device.token_rotate");

    // POST /devices/{id}/revoke
    let (row, _) = one_row(
        &c,
        empty(
            Method::POST,
            &format!("/v1/devices/{device_id}/revoke"),
            owner,
        ),
        StatusCode::OK,
    )
    .await;
    assert_eq!(row.action, "device.revoke");
    assert!(row.before.as_ref().unwrap()["revoked_at"].is_null());
    assert!(row.after.as_ref().unwrap()["revoked_at"].is_string());

    // Login, refresh, logout
    let creds = json!({"email": "owner@example.org", "password": PASSWORD});
    let (row, pair) = one_row(
        &c,
        json(Method::POST, "/v1/auth/login", &creds, None),
        StatusCode::OK,
    )
    .await;
    assert_eq!(row.action, "auth.login");
    assert_eq!(row.actor_id.as_deref(), Some(actor.as_str()));
    let rt = pair["refresh_token"].as_str().unwrap().to_owned();
    let (row, pair) = one_row(
        &c,
        json(
            Method::POST,
            "/v1/auth/refresh",
            &json!({"refresh_token": rt}),
            None,
        ),
        StatusCode::OK,
    )
    .await;
    assert_eq!(row.action, "auth.refresh");
    let rt = pair["refresh_token"].as_str().unwrap().to_owned();
    let (row, _) = one_row(
        &c,
        json(
            Method::POST,
            "/v1/auth/logout",
            &json!({"refresh_token": rt}),
            None,
        ),
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_eq!(row.action, "auth.logout");
    c.db.cleanup().await;
}

#[tokio::test]
async fn failed_and_forbidden_mutations_write_no_row() {
    let Some(c) = setup().await else { return };
    let viewer = seed_user(&c.db.pool, Some(c.org), Role::Viewer, "v@example.org").await;
    let viewer = access_token(&c.state, viewer, Some(c.org), Role::Viewer);
    let before = audit_total(&c.db.pool).await;
    for req in [
        // 403
        json(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            &json!({"name": "X"}),
            Some(&viewer),
        ),
        // 400
        json(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            &json!({"name": ""}),
            Some(&c.owner),
        ),
        // 401 (unknown code)
        json(
            Method::POST,
            "/v1/devices/register",
            &json!({"enrol_code": "NOPE", "platform": "iOS", "app_version": "1"}),
            None,
        ),
        // 409 (duplicate email)
        json(
            Method::POST,
            &format!("/v1/orgs/{}/users", c.org),
            &json!({"email": "owner@example.org", "role": "viewer", "password": "a long enough password"}),
            Some(&c.owner),
        ),
    ] {
        let (status, _, body) = send(c.app.clone(), req).await;
        assert!(status.is_client_error(), "{status}: {body}");
    }
    assert_eq!(audit_total(&c.db.pool).await, before);
    c.db.cleanup().await;
}

#[tokio::test]
async fn audit_rows_never_contain_secrets() {
    let Some(c) = setup().await else { return };
    let password = "a long enough password";
    let (_, _, user) = send(
        c.app.clone(),
        json(
            Method::POST,
            &format!("/v1/orgs/{}/users", c.org),
            &json!({"email": "new@example.org", "role": "viewer", "password": password}),
            Some(&c.owner),
        ),
    )
    .await;
    assert!(user["id"].is_string(), "{user}");
    let (_, _, code) = send(
        c.app.clone(),
        json(
            Method::POST,
            &format!("/v1/orgs/{}/devices/enrol-code", c.org),
            &json!({}),
            Some(&c.owner),
        ),
    )
    .await;
    let code = code["code"].as_str().unwrap().to_owned();
    let (_, _, device) = send(
        c.app.clone(),
        json(
            Method::POST,
            "/v1/devices/register",
            &json!({"enrol_code": code, "platform": "iOS", "app_version": "1"}),
            None,
        ),
    )
    .await;
    let device_token = device["device_token"].as_str().unwrap().to_owned();
    let (_, _, pair) = send(
        c.app.clone(),
        json(
            Method::POST,
            "/v1/auth/login",
            &json!({"email": "new@example.org", "password": password}),
            None,
        ),
    )
    .await;
    let refresh = pair["refresh_token"].as_str().unwrap().to_owned();
    let access = pair["access_token"].as_str().unwrap().to_owned();

    let dump: String = sqlx::query_scalar(
        "SELECT coalesce(string_agg(row_to_json(a)::text, '\n'), '') FROM audit_log a",
    )
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    for secret in [
        password,
        code.as_str(),
        &device_token,
        &refresh,
        &access,
        "$argon2id$",
    ] {
        assert!(
            !dump.contains(secret),
            "audit log contains a secret ({secret})"
        );
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn audit_log_is_append_only_for_every_role() {
    let Some(c) = setup().await else { return };
    sqlx::query(
        "INSERT INTO audit_log (actor_type, action, target_type) VALUES ('system', 'test', 'none')",
    )
    .execute(&c.db.pool)
    .await
    .unwrap();

    // The connection role here owns the tables (and is a superuser locally): the trigger
    // refuses anyway.
    for stmt in [
        "UPDATE audit_log SET action = 'tampered'",
        "DELETE FROM audit_log",
        "TRUNCATE audit_log",
    ] {
        let err = sqlx::query(stmt).execute(&c.db.pool).await.unwrap_err();
        assert!(err.to_string().contains("append-only"), "{stmt}: {err}");
    }

    // The application role has INSERT/SELECT, never UPDATE/DELETE/TRUNCATE.
    let privileges: Vec<(String, bool)> = sqlx::query_as(
        "SELECT p, has_table_privilege('asd_app', 'audit_log', p) \
         FROM unnest(ARRAY['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'TRUNCATE']) AS p",
    )
    .fetch_all(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(
        privileges,
        [
            ("SELECT".to_owned(), true),
            ("INSERT".to_owned(), true),
            ("UPDATE".to_owned(), false),
            ("DELETE".to_owned(), false),
            ("TRUNCATE".to_owned(), false),
        ]
    );
    let can_write_users: bool =
        sqlx::query_scalar("SELECT has_table_privilege('asd_app', 'users', 'UPDATE')")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert!(
        can_write_users,
        "the app role can still do its job elsewhere"
    );

    // As the application role itself: insert works, update is a permission error.
    let mut conn: PgConnection = c.db.pool.acquire().await.unwrap().detach();
    sqlx::query("SET ROLE asd_app")
        .execute(&mut conn)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO audit_log (actor_type, action, target_type) VALUES ('system', 'as-app', 'none')",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    let err = sqlx::query("UPDATE audit_log SET action = 'x'")
        .execute(&mut conn)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("permission denied"), "{err}");
    conn.close().await.ok();

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(rows, 2);
    c.db.cleanup().await;
}

#[tokio::test]
async fn audit_list_is_org_scoped_newest_first_and_paginated() {
    let Some(c) = setup().await else { return };
    let other = seed_org(&c.db.pool, "Clinic B").await;
    for (org, action) in [
        (Some(c.org), "a.1"),
        (Some(other), "b.1"),
        (Some(c.org), "a.2"),
        (None, "global"),
        (Some(c.org), "a.3"),
    ] {
        sqlx::query(
            "INSERT INTO audit_log (actor_type, org_id, action, target_type) VALUES ('system', $1, $2, 'none')",
        )
        .bind(org)
        .bind(action)
        .execute(&c.db.pool)
        .await
        .unwrap();
    }
    let uri = format!("/v1/orgs/{}/audit?limit=2", c.org);
    let (status, _, page1) = send(c.app.clone(), empty(Method::GET, &uri, Some(&c.owner))).await;
    assert_eq!(status, StatusCode::OK, "{page1}");
    let actions = |p: &Value| -> Vec<String> {
        p["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["action"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(actions(&page1), ["a.3", "a.2"]);
    let item = &page1["items"][0];
    assert_eq!(item["actor_type"], "system");
    assert_eq!(item["org_id"], c.org.to_string());
    assert!(item["id"].is_string() && item["ts"].is_string());

    let cursor = page1["next_cursor"].as_str().unwrap();
    let uri = format!("/v1/orgs/{}/audit?limit=2&cursor={cursor}", c.org);
    let (_, _, page2) = send(c.app.clone(), empty(Method::GET, &uri, Some(&c.owner))).await;
    assert_eq!(actions(&page2), ["a.1"]);
    assert_eq!(page2["next_cursor"], Value::Null);
    c.db.cleanup().await;
}

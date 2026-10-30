//! owner: WP-B.2 (bootstrap follow-up)
//!
//! First-run operator path (`asd-backend admin create-org | create-user`) and the contract
//! fixes that came with it: `User.org_id` is `null` for the superadmin, `invited` is a
//! user status, and withdrawn participants have their own problem slug (tested in
//! `sessions.rs`). Needs `DATABASE_URL` (skipped otherwise).

mod common;

use std::io::Write;
use std::process::{Command, Output, Stdio};

use asd_backend::admin::{self, AdminError, NewOrg, NewUser};
use asd_contract::api_v1::{OrgKind, Role};
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const OWNER_PASSWORD: &str = "first owner password";

/// `(action, actor_type, target_id, org_id, after)` of one `audit_log` row.
type AuditRow = (String, String, Option<String>, Option<Uuid>, Value);

fn new_org(name: &str) -> NewOrg {
    NewOrg {
        name: name.into(),
        kind: OrgKind::Clinic,
        retention_months: None,
        force: false,
    }
}

fn new_user(email: &str, role: Role, org_id: Option<Uuid>) -> NewUser {
    NewUser {
        email: email.into(),
        role,
        org_id,
        force: false,
    }
}

async fn login(app: &axum::Router, email: &str, password: &str) -> (StatusCode, Value) {
    let (status, _, body) = send(
        app.clone(),
        json(
            Method::POST,
            "/v1/auth/login",
            &json!({"email": email, "password": password}),
            None,
        ),
    )
    .await;
    (status, body)
}

async fn user_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// `DATABASE_URL` with its database name replaced by the test database's.
fn test_db_url(db: &TestDb) -> String {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b.to_owned(), format!("?{q}")),
        None => (url.clone(), String::new()),
    };
    let (server, _) = base.rsplit_once('/').expect("URL has a database path");
    format!("{server}/{}{query}", db.name)
}

/// Runs the real binary with only `DATABASE_URL` (and `extra`) in its environment.
fn run_admin(db: &TestDb, args: &[&str], stdin: &str, extra: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_asd-backend"));
    cmd.env_clear()
        .env("DATABASE_URL", test_db_url(db))
        .arg("admin")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn asd-backend");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().expect("asd-backend exits")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[tokio::test]
async fn first_run_sequence_create_org_create_owner_login_enrol_code() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let state = state_with_pool(db.pool.clone());
    let app = router(state);

    // 1. create-org prints only the new id on stdout.
    let out = run_admin(
        &db,
        &["create-org", "--name", "First Clinic", "--kind", "clinic"],
        "",
        &[],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let org: Uuid = stdout(&out).parse().expect("stdout is the org id");

    // 2. create-user reads the password from stdin (never argv).
    let out = run_admin(
        &db,
        &[
            "create-user",
            "--email",
            "Owner@Example.org",
            "--role",
            "owner",
            "--org",
            &org.to_string(),
        ],
        &format!("{OWNER_PASSWORD}\n"),
        &[],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let owner: Uuid = stdout(&out).parse().expect("stdout is the user id");
    assert!(!stderr(&out).contains(OWNER_PASSWORD));

    // Both writes are audited as system actions.
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT action, actor_type, target_id, org_id, after FROM audit_log ORDER BY id",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].0, "org.create");
    assert_eq!(rows[0].1, "system");
    assert_eq!(rows[0].2, Some(org.to_string()));
    assert_eq!(rows[0].3, Some(org));
    assert_eq!(rows[0].4["name"], "First Clinic");
    assert_eq!(rows[1].0, "user.create");
    assert_eq!(rows[1].1, "system");
    assert_eq!(rows[1].2, Some(owner.to_string()));
    assert_eq!(rows[1].3, Some(org));
    assert_eq!(rows[1].4["role"], "owner");
    assert_eq!(rows[1].4["email"], "owner@example.org");
    assert!(!rows[1].4.to_string().contains(OWNER_PASSWORD));

    // 3. The owner signs in.
    let (status, pair) = login(&app, "owner@example.org", OWNER_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{pair}");
    assert_eq!(pair["user"]["org_id"], org.to_string());
    assert_eq!(pair["user"]["role"], "owner");
    let access = pair["access_token"].as_str().unwrap();

    // 4. ...and issues the first enrol code through the API.
    let (status, _, code) = send(
        app.clone(),
        json(
            Method::POST,
            &format!("/v1/orgs/{org}/devices/enrol-code"),
            &json!({}),
            Some(access),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{code}");
    assert!(code["code"].as_str().is_some_and(|c| !c.is_empty()));
    db.cleanup().await;
}

#[tokio::test]
async fn binary_takes_the_password_from_the_env_and_refuses_it_in_argv() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let out = run_admin(
        &db,
        &[
            "create-user",
            "--email",
            "root@example.org",
            "--role",
            "superadmin",
            "--password",
            "argv password 123",
        ],
        "",
        &[],
    );
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(
        stderr(&out).contains(admin::PASSWORD_ENV),
        "{}",
        stderr(&out)
    );
    assert!(!stderr(&out).contains("argv password 123"));
    assert_eq!(user_count(&db.pool).await, 0);

    let out = run_admin(
        &db,
        &[
            "create-user",
            "--email",
            "root@example.org",
            "--role",
            "superadmin",
        ],
        "",
        &[(admin::PASSWORD_ENV, "env root password")],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let app = router(state_with_pool(db.pool.clone()));
    let (status, pair) = login(&app, "root@example.org", "env root password").await;
    assert_eq!(status, StatusCode::OK, "{pair}");
    db.cleanup().await;
}

#[tokio::test]
async fn binary_exit_codes_distinguish_usage_conflict_and_success() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let args = [
        "create-user",
        "--email",
        "root@example.org",
        "--role",
        "superadmin",
    ];
    let out = run_admin(&db, &args, "root password one\n", &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    // Same e-mail again: a conflict (3), with a message naming the address.
    let out = run_admin(&db, &args, "root password one\n", &[]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(stderr(&out).contains("already exists"), "{}", stderr(&out));
    // A short password is a usage error (2).
    let out = run_admin(
        &db,
        &[
            "create-user",
            "--email",
            "x@example.org",
            "--role",
            "superadmin",
        ],
        "short\n",
        &[],
    );
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(user_count(&db.pool).await, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn a_second_superadmin_needs_force() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let first = new_user("root@example.org", Role::Superadmin, None);
    admin::create_user(&db.pool, &first, "root password one".into())
        .await
        .unwrap();
    let second = new_user("root2@example.org", Role::Superadmin, None);
    let err = admin::create_user(&db.pool, &second, "root password two".into())
        .await
        .unwrap_err();
    assert!(matches!(err, AdminError::SuperadminExists), "{err}");
    assert_eq!(err.exit_code(), 3);
    assert_eq!(user_count(&db.pool).await, 1);
    assert_eq!(audit_count(&db.pool, "user.create").await, 1);

    let forced = NewUser {
        force: true,
        ..second
    };
    admin::create_user(&db.pool, &forced, "root password two".into())
        .await
        .unwrap();
    assert_eq!(user_count(&db.pool).await, 2);
    let after: Value = sqlx::query_scalar(
        "SELECT after FROM audit_log WHERE action = 'user.create' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(after["forced"], true, "{after}");
    db.cleanup().await;
}

#[tokio::test]
async fn an_existing_email_is_a_clear_error_and_changes_nothing() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let org = admin::create_org(&db.pool, &new_org("Clinic A"))
        .await
        .unwrap();
    let owner = new_user("owner@example.org", Role::Owner, Some(org));
    let id = admin::create_user(&db.pool, &owner, "owner password one".into())
        .await
        .unwrap();
    let audit_before = audit_total(&db.pool).await;

    // Case-insensitively the same address, even with --force.
    let again = NewUser {
        email: " OWNER@example.org ".into(),
        force: true,
        ..owner
    };
    let err = admin::create_user(&db.pool, &again, "another password".into())
        .await
        .unwrap_err();
    match &err {
        AdminError::EmailTaken {
            email,
            id: existing,
        } => {
            assert_eq!(email, "owner@example.org");
            assert_eq!(*existing, id);
        }
        other => panic!("expected EmailTaken, got {other}"),
    }
    assert!(err.to_string().contains("owner@example.org"));
    assert_eq!(user_count(&db.pool).await, 1);
    assert_eq!(audit_total(&db.pool).await, audit_before);

    // The original password still works: nothing was overwritten.
    let app = router(state_with_pool(db.pool.clone()));
    let (status, _) = login(&app, "owner@example.org", "owner password one").await;
    assert_eq!(status, StatusCode::OK);
    db.cleanup().await;
}

#[tokio::test]
async fn org_rules_for_owner_and_superadmin() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let missing = Uuid::new_v4();
    let err = admin::create_user(
        &db.pool,
        &new_user("o@example.org", Role::Owner, Some(missing)),
        "owner password one".into(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, AdminError::OrgNotFound(id) if id == missing),
        "{err}"
    );

    let err = admin::create_user(
        &db.pool,
        &new_user("o@example.org", Role::Owner, None),
        "owner password one".into(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.exit_code(), 2, "{err}");

    let org = admin::create_org(&db.pool, &new_org("Clinic A"))
        .await
        .unwrap();
    let err = admin::create_user(
        &db.pool,
        &new_user("root@example.org", Role::Superadmin, Some(org)),
        "root password one".into(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.exit_code(), 2, "{err}");
    assert_eq!(user_count(&db.pool).await, 0);

    // Same org name twice needs --force (protects against a re-run creating a twin).
    let err = admin::create_org(&db.pool, &new_org("clinic a"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, AdminError::OrgNameTaken { id, .. } if id == org),
        "{err}"
    );
    let twin = NewOrg {
        force: true,
        ..new_org("Clinic A")
    };
    assert_ne!(admin::create_org(&db.pool, &twin).await.unwrap(), org);
    db.cleanup().await;
}

#[tokio::test]
async fn superadmin_login_returns_a_null_org_id() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    seed_user(&db.pool, None, Role::Superadmin, "root@example.org").await;
    let app = router(state_with_pool(db.pool.clone()));
    let (status, pair) = login(&app, "root@example.org", PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{pair}");
    assert_eq!(pair["user"]["role"], "superadmin");
    assert!(
        pair["user"].as_object().unwrap().contains_key("org_id"),
        "{pair}"
    );
    assert!(pair["user"]["org_id"].is_null(), "{pair}");

    let (status, _, refreshed) = send(
        app.clone(),
        json(
            Method::POST,
            "/v1/auth/refresh",
            &json!({"refresh_token": pair["refresh_token"]}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refreshed}");
    assert!(refreshed["user"]["org_id"].is_null(), "{refreshed}");
    db.cleanup().await;
}

#[tokio::test]
async fn invited_users_list_as_invited_cannot_sign_in_and_are_not_set_by_patch() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let owner = seed_user(&db.pool, Some(org), Role::Owner, "owner@example.org").await;
    let invited = seed_user(&db.pool, Some(org), Role::Viewer, "invited@example.org").await;
    sqlx::query("UPDATE users SET status = 'invited' WHERE id = $1")
        .bind(invited)
        .execute(&db.pool)
        .await
        .unwrap();
    let token = access_token(&state, owner, Some(org), Role::Owner);

    let (status, _, page) = send(
        app.clone(),
        empty(Method::GET, &format!("/v1/orgs/{org}/users"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let item = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == invited.to_string())
        .expect("invited user listed");
    assert_eq!(item["status"], "invited");

    let (status, _) = login(&app, "invited@example.org", PASSWORD).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // `invited` is set by an invitation flow, never by PATCH.
    let (status, _, resp) = send(
        app.clone(),
        json(
            Method::PATCH,
            &format!("/v1/orgs/{org}/users/{owner}"),
            &json!({"status": "invited"}),
            Some(&access_token(
                &state,
                seed_user(&db.pool, Some(org), Role::Owner, "owner2@example.org").await,
                Some(org),
                Role::Owner,
            )),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert_eq!(slug(&resp), "validation-failed");
    assert_eq!(resp["invalid_params"][0]["name"], "status");

    // Activating an invited user works and reads back as active.
    let (status, _, resp) = send(
        app.clone(),
        json(
            Method::PATCH,
            &format!("/v1/orgs/{org}/users/{invited}"),
            &json!({"status": "active"}),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["status"], "active");
    db.cleanup().await;
}

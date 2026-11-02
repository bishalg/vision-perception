//! owner: WP-B.24
//!
//! `asd-backend admin reset-password --user <id>` (emergency access, 164.312(a)(2)(ii)):
//! the generated password logs in and the old one fails; refresh tokens are revoked; TOTP
//! is untouched; the audit row has no secret; short `ASD_ADMIN_PASSWORD`, disabled,
//! invited and unknown users exit 2 and change nothing. Needs `DATABASE_URL` (skipped
//! otherwise).

mod common;

use std::process::{Command, Output, Stdio};

use asd_contract::api_v1::Role;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

const TOTP_SECRET: &str = "JBSWY3DPEHPK3PXP";

type TotpState = (bool, Option<chrono::DateTime<chrono::Utc>>);

fn test_db_url(db: &TestDb) -> String {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b.to_owned(), format!("?{q}")),
        None => (url.clone(), String::new()),
    };
    let (server, _) = base.rsplit_once('/').expect("URL has a database path");
    format!("{server}/{}{query}", db.name)
}

fn run(db: &TestDb, user: &str, extra: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_asd-backend"));
    cmd.env_clear()
        .env("DATABASE_URL", test_db_url(db))
        .args(["admin", "reset-password", "--user", user])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        cmd.env(k, v);
    }
    cmd.spawn().unwrap().wait_with_output().unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

async fn login(app: &axum::Router, email: &str, password: &str) -> (StatusCode, Value) {
    let (s, _, b) = send(
        app.clone(),
        json(
            Method::POST,
            "/v1/auth/login",
            &json!({"email": email, "password": password}),
            None,
        ),
    )
    .await;
    (s, b)
}

async fn refresh(app: &axum::Router, token: &str) -> StatusCode {
    send(
        app.clone(),
        json(
            Method::POST,
            "/v1/auth/refresh",
            &json!({"refresh_token": token}),
            None,
        ),
    )
    .await
    .0
}

async fn pw_hash(db: &TestDb, id: Uuid) -> String {
    sqlx::query_scalar("SELECT pw_hash FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap()
}

async fn totp_state(db: &TestDb, id: Uuid) -> TotpState {
    sqlx::query_as("SELECT totp_secret_ct IS NULL, totp_required_by FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn generated_password_replaces_old_revokes_refresh_keeps_totp_audits_without_secret() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic").await;
    let email = "owner@example.org";
    let id = seed_user(&db.pool, Some(org), Role::Owner, email).await;
    let (s, body) = login(&app, email, PASSWORD).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let old_refresh = body["refresh_token"].as_str().unwrap().to_owned();
    enrol_totp(&state, id, TOTP_SECRET).await;
    let totp_before = totp_state(&db, id).await;
    let old_hash = pw_hash(&db, id).await;

    let o = run(&db, &id.to_string(), &[]);
    assert!(o.status.success(), "{}", err(&o));
    let temp = out(&o);
    assert_eq!(temp.len(), 20, "stdout is the temporary password only");
    assert!(err(&o).contains("15 minutes"));
    assert!(!err(&o).contains(&temp));

    let new_hash = pw_hash(&db, id).await;
    assert_ne!(new_hash, old_hash);
    assert_eq!(refresh(&app, &old_refresh).await, StatusCode::UNAUTHORIZED);
    // The enrolled user still needs the second factor: password right, code missing.
    let (s, b) = login(&app, email, &temp).await;
    assert_eq!(slug(&b), "totp-required", "{s} {b}");
    let (s, _) = login(&app, email, PASSWORD).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let totp_after = totp_state(&db, id).await;
    assert_eq!(totp_before, totp_after);
    assert!(!totp_after.0, "TOTP secret still present");

    let rows: Vec<(String, Option<Uuid>, Value)> = sqlx::query_as(
        "SELECT actor_type, org_id, after FROM audit_log WHERE action = 'user.password_reset'",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "system");
    assert_eq!(rows[0].1, Some(org));
    assert_eq!(
        rows[0].2,
        json!({"actor": "operator", "via": "cli", "role": "owner", "generated": true})
    );
    let dump: String = sqlx::query_scalar(
        "SELECT coalesce(string_agg(row_to_json(a)::text, ' '), '') FROM audit_log a",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(!dump.contains(&temp));
    assert!(!dump.contains(&new_hash));
    db.cleanup().await;
}

#[tokio::test]
async fn supplied_password_is_used_and_not_echoed() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let app = router(state_with_pool(db.pool.clone()));
    let org = seed_org(&db.pool, "Clinic").await;
    let id = seed_user(&db.pool, Some(org), Role::Owner, "o@example.org").await;
    let chosen = "my own long passphrase";
    let o = run(&db, &id.to_string(), &[("ASD_ADMIN_PASSWORD", chosen)]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(out(&o), id.to_string());
    assert!(!err(&o).contains(chosen));
    assert_eq!(login(&app, "o@example.org", chosen).await.0, StatusCode::OK);
    let after: Value =
        sqlx::query_scalar("SELECT after FROM audit_log WHERE action = 'user.password_reset'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(after["generated"], json!(false));
    db.cleanup().await;
}

#[tokio::test]
async fn short_password_disabled_invited_and_unknown_users_exit_2_and_change_nothing() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let org = seed_org(&db.pool, "Clinic").await;
    let active = seed_user(&db.pool, Some(org), Role::Owner, "a@example.org").await;
    let disabled = seed_user(&db.pool, Some(org), Role::Admin, "d@example.org").await;
    let invited = seed_user(&db.pool, Some(org), Role::Admin, "i@example.org").await;
    for (id, status) in [(disabled, "disabled"), (invited, "invited")] {
        sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(status)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    let before = (
        pw_hash(&db, active).await,
        pw_hash(&db, disabled).await,
        pw_hash(&db, invited).await,
    );

    let o = run(&db, &active.to_string(), &[("ASD_ADMIN_PASSWORD", "short")]);
    assert_eq!(o.status.code(), Some(2), "{}", err(&o));
    for id in [disabled, invited] {
        let o = run(&db, &id.to_string(), &[]);
        assert_eq!(o.status.code(), Some(2), "{}", err(&o));
        assert!(out(&o).is_empty());
    }
    let o = run(&db, &Uuid::new_v4().to_string(), &[]);
    assert_eq!(o.status.code(), Some(2), "{}", err(&o));
    let o = run(&db, "not-a-uuid", &[]);
    assert_eq!(o.status.code(), Some(2));

    assert_eq!(
        before,
        (
            pw_hash(&db, active).await,
            pw_hash(&db, disabled).await,
            pw_hash(&db, invited).await
        )
    );
    assert_eq!(audit_count(&db.pool, "user.password_reset").await, 0);
    db.cleanup().await;
}

//! owner: WP-B.17
//!
//! TRS-1, TRS-2 (acceptance 12): `POST /v1/orgs/{id}/users/{user_id}/totp/reset`. An owner
//! or admin re-authenticates (SIGN-5 step-up) and clears another user's TOTP: columns
//! NULL, refresh families revoked, `user.totp_reset_by_admin` audited with the target and
//! the reason only, `totp_required_by` untouched. Admin on owner, self, viewer, other org,
//! wrong password and a bad reason change nothing. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::app::AppState;
use asd_backend::clinical::reauth;
use asd_contract::api_v1::Role;
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

const TARGET_SECRET: &str = "JBSWY3DPEHPK3PXP";
const CALLER_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const REASON: &str = "lost phone, verified in person at the front desk";

/// `(actor_type, actor_id, target_id, org_id, after)` of one `audit_log` row.
type AuditRow = (String, Option<String>, Option<String>, Option<Uuid>, Value);

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
    org: Uuid,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic").await;
    Some(Ctx {
        db,
        state,
        app,
        org,
    })
}

impl Ctx {
    async fn user(&self, org: Option<Uuid>, role: Role) -> (Uuid, String, String) {
        let email = format!("{}-{}@example.org", role_str(role), Uuid::new_v4().simple());
        let id = seed_user(&self.db.pool, org, role, &email).await;
        (id, access_token(&self.state, id, org, role), email)
    }

    async fn reset(
        &self,
        org: Uuid,
        target: Uuid,
        body: Value,
        bearer: &str,
    ) -> (StatusCode, Value) {
        let (s, _, b) = send(
            self.app.clone(),
            json(
                Method::POST,
                &format!("/v1/orgs/{org}/users/{target}/totp/reset"),
                &body,
                Some(bearer),
            ),
        )
        .await;
        (s, b)
    }

    async fn login(&self, email: &str, code: Option<String>) -> (StatusCode, Value) {
        let mut body = json!({"email": email, "password": PASSWORD});
        if let Some(code) = code {
            body["totp_code"] = json!(code);
        }
        let (s, _, b) = send(
            self.app.clone(),
            json(Method::POST, "/v1/auth/login", &body, None),
        )
        .await;
        (s, b)
    }

    async fn refresh(&self, token: &str) -> StatusCode {
        send(
            self.app.clone(),
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

    /// `(totp_secret_ct IS NULL, last_totp_step IS NULL, totp_pending_ct IS NULL,
    /// totp_pending_expires_at IS NULL, totp_required_by)`.
    async fn totp_state(
        &self,
        user: Uuid,
    ) -> (
        bool,
        bool,
        bool,
        bool,
        Option<chrono::DateTime<chrono::Utc>>,
    ) {
        sqlx::query_as(
            "SELECT totp_secret_ct IS NULL, last_totp_step IS NULL, totp_pending_ct IS NULL,
                    totp_pending_expires_at IS NULL, totp_required_by
             FROM users WHERE id = $1",
        )
        .bind(user)
        .fetch_one(&self.db.pool)
        .await
        .unwrap()
    }
}

fn now_code(secret: &str) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    reauth::totp_code(secret, t).unwrap()
}

fn body(password: &str) -> Value {
    json!({"password": password, "reason": REASON})
}

#[tokio::test]
async fn admin_resets_a_clinician_columns_cleared_refresh_revoked_audited() {
    let Some(c) = setup().await else { return };
    let (admin, admin_token, _) = c.user(Some(c.org), Role::Admin).await;
    let (target, _, target_email) = c.user(Some(c.org), Role::Clinician).await;
    enrol_totp(&c.state, target, TARGET_SECRET).await;
    // A deadline in the past: the reset must not grant a new grace period.
    let deadline = chrono::Utc::now() - chrono::Duration::days(1);
    sqlx::query(
        "UPDATE users SET totp_required_by = date_trunc('second', $2::timestamptz),
                totp_pending_ct = '\\x00', totp_pending_kek_id = 'kek-x',
                totp_pending_expires_at = now() + interval '10 minutes'
         WHERE id = $1",
    )
    .bind(target)
    .bind(deadline)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (_, _, _, _, required_before) = c.totp_state(target).await;

    let (status, login) = c.login(&target_email, Some(now_code(TARGET_SECRET))).await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let refresh_token = login["refresh_token"].as_str().unwrap().to_owned();

    let (status, resp) = c.reset(c.org, target, body(PASSWORD), &admin_token).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{resp}");

    let (secret, step, pending, pending_exp, required_after) = c.totp_state(target).await;
    assert!(secret && step && pending && pending_exp);
    assert_eq!(
        required_after, required_before,
        "totp_required_by untouched"
    );
    assert_eq!(
        c.refresh(&refresh_token).await,
        StatusCode::UNAUTHORIZED,
        "the old refresh family is revoked"
    );
    // The password alone signs in again (enrolment then required by policy).
    let (status, login) = c.login(&target_email, None).await;
    assert_eq!(status, StatusCode::OK, "{login}");

    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT actor_type, actor_id, target_id, org_id, after FROM audit_log
             WHERE action = 'user.totp_reset_by_admin'",
    )
    .fetch_all(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    let (actor_type, actor_id, target_id, org_id, after) = &rows[0];
    assert_eq!(actor_type, "user");
    assert_eq!(actor_id.as_deref(), Some(admin.to_string().as_str()));
    assert_eq!(target_id.as_deref(), Some(target.to_string().as_str()));
    assert_eq!(*org_id, Some(c.org));
    assert_eq!(
        after,
        &json!({"target_id": target, "reason": REASON}),
        "the target and the reason, nothing else"
    );
    let text = after.to_string();
    assert!(!text.contains(TARGET_SECRET) && !text.contains(PASSWORD));
    c.db.cleanup().await;
}

#[tokio::test]
async fn owner_with_totp_needs_a_code_and_may_reset_an_admin() {
    let Some(c) = setup().await else { return };
    let (owner, owner_token, _) = c.user(Some(c.org), Role::Owner).await;
    enrol_totp(&c.state, owner, CALLER_SECRET).await;
    let (target, _, _) = c.user(Some(c.org), Role::Admin).await;
    enrol_totp(&c.state, target, TARGET_SECRET).await;

    let (status, resp) = c.reset(c.org, target, body(PASSWORD), &owner_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no code: {resp}");
    assert_eq!(slug(&resp), "reauthentication-failed");
    assert!(!c.totp_state(target).await.0, "nothing changed");

    let mut with_code = body(PASSWORD);
    with_code["totp_code"] = json!(now_code(CALLER_SECRET));
    let (status, resp) = c.reset(c.org, target, with_code, &owner_token).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{resp}");
    assert!(c.totp_state(target).await.0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn admin_on_owner_and_self_are_forbidden() {
    let Some(c) = setup().await else { return };
    let (admin, admin_token, _) = c.user(Some(c.org), Role::Admin).await;
    let (owner, owner_token, _) = c.user(Some(c.org), Role::Owner).await;
    for (who, target, token) in [
        ("admin on owner", owner, &admin_token),
        ("admin on self", admin, &admin_token),
        ("owner on self", owner, &owner_token),
    ] {
        enrol_totp(&c.state, target, TARGET_SECRET).await;
        let (status, resp) = c.reset(c.org, target, body(PASSWORD), token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who}: {resp}");
        assert_eq!(slug(&resp), "totp-reset-forbidden", "{who}");
        assert!(!c.totp_state(target).await.0, "{who}: nothing changed");
    }
    assert_eq!(audit_count(&c.db.pool, "user.totp_reset_by_admin").await, 0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn wrong_password_is_401_audited_and_changes_nothing() {
    let Some(c) = setup().await else { return };
    let (admin, admin_token, _) = c.user(Some(c.org), Role::Admin).await;
    let (target, _, _) = c.user(Some(c.org), Role::Clinician).await;
    enrol_totp(&c.state, target, TARGET_SECRET).await;

    let (status, resp) = c
        .reset(c.org, target, body("not the password at all"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{resp}");
    assert_eq!(slug(&resp), "reauthentication-failed");
    assert!(!c.totp_state(target).await.0, "nothing changed");
    assert_eq!(audit_count(&c.db.pool, "user.totp_reset_by_admin").await, 0);

    let rows: Vec<(Option<String>, Option<String>, Value)> = sqlx::query_as(
        "SELECT actor_id, target_id, after FROM audit_log WHERE action = 'user.totp_reset_failed'",
    )
    .fetch_all(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0.as_deref(), Some(admin.to_string().as_str()));
    assert_eq!(rows[0].1.as_deref(), Some(target.to_string().as_str()));
    assert_eq!(rows[0].2["reason"], "reauthentication-failed");
    assert!(!rows[0].2.to_string().contains("not the password"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn viewer_403_other_org_404_bad_reason_400() {
    let Some(c) = setup().await else { return };
    let (_, viewer_token, _) = c.user(Some(c.org), Role::Viewer).await;
    let (_, admin_token, _) = c.user(Some(c.org), Role::Admin).await;
    let (target, _, _) = c.user(Some(c.org), Role::Clinician).await;
    enrol_totp(&c.state, target, TARGET_SECRET).await;

    let (status, resp) = c.reset(c.org, target, body(PASSWORD), &viewer_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "viewer: {resp}");
    assert_eq!(slug(&resp), "forbidden");

    // Another org's user, by its own org path (404 before the role check) and by ours.
    let other = seed_org(&c.db.pool, "Other").await;
    let (stranger, _, _) = c.user(Some(other), Role::Clinician).await;
    enrol_totp(&c.state, stranger, TARGET_SECRET).await;
    let (status, _) = c.reset(other, stranger, body(PASSWORD), &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = c.reset(c.org, stranger, body(PASSWORD), &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!c.totp_state(stranger).await.0);

    for reason in ["too short", &"x".repeat(201), "line\nbreak in a reason"] {
        let (status, resp) = c
            .reset(
                c.org,
                target,
                json!({"password": PASSWORD, "reason": reason}),
                &admin_token,
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{reason:?}: {resp}");
        assert_eq!(slug(&resp), "validation-failed");
    }
    assert!(!c.totp_state(target).await.0, "nothing changed");
    c.db.cleanup().await;
}

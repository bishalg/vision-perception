//! owner: WP-B.16
//!
//! AUTH-11..AUTH-14 (ADR-0010, acceptance 25): mandatory TOTP per role through the real
//! router. Needs `DATABASE_URL` (skipped otherwise). A passed deadline is produced with
//! SQL (`UPDATE users SET totp_required_by = now() - interval '1 day'`); there is no test
//! hook in the service for it (trap 14).

mod common;

use asd_backend::app::AppState;
use asd_backend::auth::ManualClock;
use asd_backend::clinical::reauth::totp_code;
use asd_contract::api_v1::Role;
use axum::Router;
use chrono::{DateTime, Duration, Utc};
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

const ME: &str = "/v1/users/me";
const ENROL: &str = "/v1/users/me/totp/enrol";
const CONFIRM: &str = "/v1/users/me/totp/confirm";
const TOTP: &str = "/v1/users/me/totp";

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
    org: Uuid,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let mut state = state_with_pool(db.pool.clone());
    // Every request of a test falls into one rate-limit window.
    state.clock = ManualClock::new();
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    Some(Ctx {
        db,
        state,
        app,
        org,
    })
}

impl Ctx {
    async fn user(&self, role: Role, email: &str) -> (Uuid, String) {
        let org = (role != Role::Superadmin).then_some(self.org);
        let id = seed_user(&self.db.pool, org, role, email).await;
        let bearer = access_token(&self.state, id, org, role);
        (id, bearer)
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        bearer: &str,
    ) -> (StatusCode, Value) {
        let request = match body {
            Some(b) => json(method, uri, &b, Some(bearer)),
            None => empty(method, uri, Some(bearer)),
        };
        let (status, _, body) = send(self.app.clone(), request).await;
        (status, body)
    }

    async fn login(&self, email: &str) -> (StatusCode, Value) {
        let body = json!({"email": email, "password": PASSWORD});
        let (status, _, body) = send(
            self.app.clone(),
            from_ip(
                json(Method::POST, "/v1/auth/login", &body, None),
                "198.51.100.9",
            ),
        )
        .await;
        (status, body)
    }

    async fn me(&self, bearer: &str) -> Value {
        let (status, body) = self.call(Method::GET, ME, None, bearer).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn required_by(&self, user: Uuid) -> Option<DateTime<Utc>> {
        sqlx::query_scalar("SELECT totp_required_by FROM users WHERE id = $1")
            .bind(user)
            .fetch_one(&self.db.pool)
            .await
            .unwrap()
    }

    async fn expire_deadline(&self, user: Uuid) {
        sqlx::query("UPDATE users SET totp_required_by = now() - interval '1 day' WHERE id = $1")
            .bind(user)
            .execute(&self.db.pool)
            .await
            .unwrap();
    }

    fn sessions(&self) -> String {
        format!("/v1/orgs/{}/sessions", self.org)
    }

    fn org_path(&self) -> String {
        format!("/v1/orgs/{}", self.org)
    }

    async fn audit_after(&self, action: &str) -> Vec<Value> {
        sqlx::query_scalar(
            "SELECT COALESCE(after, 'null'::jsonb) FROM audit_log WHERE action = $1 ORDER BY id",
        )
        .bind(action)
        .fetch_all(&self.db.pool)
        .await
        .unwrap()
    }

    /// Enrols and confirms with the caller's own token; returns the seed.
    async fn enrol_and_confirm(&self, bearer: &str) -> String {
        let (status, body) = self
            .call(
                Method::POST,
                ENROL,
                Some(json!({"password": PASSWORD})),
                bearer,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let secret = body["secret"].as_str().unwrap().to_owned();
        let (status, body) = self
            .call(
                Method::POST,
                CONFIRM,
                Some(json!({"totp_code": code_for(&secret, 0)})),
                bearer,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        secret
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The code `steps` 30-second steps from now.
fn code_for(secret: &str, steps: i64) -> String {
    let t = i64::try_from(now_unix()).unwrap() + steps * 30;
    totp_code(secret, u64::try_from(t).unwrap()).unwrap()
}

fn ts(v: &Value) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(v.as_str().unwrap_or_else(|| panic!("not a string: {v}")))
        .unwrap()
        .with_timezone(&Utc)
}

fn assert_about_14_days_ahead(deadline: DateTime<Utc>) {
    let ahead = deadline - Utc::now();
    assert!(
        ahead > Duration::days(14) - Duration::minutes(5) && ahead <= Duration::days(14),
        "{deadline} is not 14 days ahead"
    );
}

// ---- AUTH-12: the deadline is set lazily, once, for required roles ----------------------

#[tokio::test]
async fn login_sets_the_deadline_once_for_a_required_role() {
    let Some(c) = setup().await else { return };
    let (admin, bearer) = c.user(Role::Admin, "admin@example.org").await;
    assert_eq!(c.required_by(admin).await, None, "no back-fill");

    let me = c.me(&bearer).await;
    assert_eq!(me["totp_enforcement"], "grace", "{me}");
    assert!(me["totp_required_by"].is_null(), "{me}");

    let (status, body) = c.login("admin@example.org").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stored = c.required_by(admin).await.expect("login set the deadline");
    assert_about_14_days_ahead(stored);

    let me = c.me(&bearer).await;
    assert_eq!(me["totp_enforcement"], "grace", "{me}");
    let shown = ts(&me["totp_required_by"]);
    assert!(
        (shown - stored).num_milliseconds().abs() < 1,
        "{shown} vs {stored}"
    );

    let logins = c.audit_after("auth.login").await;
    assert_eq!(logins.len(), 1);
    assert_eq!(
        logins[0]["totp_required_by"], me["totp_required_by"],
        "{}",
        logins[0]
    );
    assert!(logins[0]["refresh_family"].is_string());
    assert_eq!(
        audit_count(&c.db.pool, "auth.login_totp_enrolment_required").await,
        0
    );

    // A second login does not move it, and its audit row does not carry it.
    let (status, body) = c.login("admin@example.org").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.required_by(admin).await, Some(stored));
    let logins = c.audit_after("auth.login").await;
    assert_eq!(logins.len(), 2);
    assert!(logins[1].get("totp_required_by").is_none(), "{}", logins[1]);
    c.db.cleanup().await;
}

#[tokio::test]
async fn login_leaves_unlisted_and_enrolled_users_alone() {
    let Some(c) = setup().await else { return };
    let (viewer, bearer) = c.user(Role::Viewer, "viewer@example.org").await;
    let (status, body) = c.login("viewer@example.org").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.required_by(viewer).await, None);
    let me = c.me(&bearer).await;
    assert_eq!(me["totp_enforcement"], "not_required", "{me}");
    assert!(me["totp_required_by"].is_null());

    let (owner, bearer) = c.user(Role::Owner, "owner@example.org").await;
    let secret = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
    enrol_totp(&c.state, owner, secret).await;
    let body = json!({"email": "owner@example.org", "password": PASSWORD,
                      "totp_code": code_for(secret, 0)});
    let (status, _, body) = send(
        c.app.clone(),
        from_ip(
            json(Method::POST, "/v1/auth/login", &body, None),
            "198.51.100.10",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.required_by(owner).await, None, "enrolled: no deadline");
    assert_eq!(c.me(&bearer).await["totp_enforcement"], "not_required");
    c.db.cleanup().await;
}

#[tokio::test]
async fn creating_a_user_in_a_required_role_sets_the_deadline() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.user(Role::Owner, "owner@example.org").await;
    let users = format!("/v1/orgs/{}/users", c.org);
    let mut ids = Vec::new();
    for (email, role) in [("c@example.org", "clinician"), ("v@example.org", "viewer")] {
        let (status, body) = c
            .call(
                Method::POST,
                &users,
                Some(
                    json!({"email": email, "password": "a long enough password 1",
                            "role": role}),
                ),
                &owner,
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        ids.push(Uuid::parse_str(body["id"].as_str().unwrap()).unwrap());
    }
    assert_about_14_days_ahead(c.required_by(ids[0]).await.expect("clinician"));
    assert_eq!(c.required_by(ids[1]).await, None, "viewer");
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_role_change_into_a_required_role_sets_the_deadline_and_back_leaves_it() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.user(Role::Owner, "owner@example.org").await;
    let (viewer, viewer_bearer) = c.user(Role::Viewer, "viewer@example.org").await;
    let uri = format!("/v1/orgs/{}/users/{viewer}", c.org);

    let (status, body) = c
        .call(
            Method::PATCH,
            &uri,
            Some(json!({"display_name": "V"})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.required_by(viewer).await, None, "no role change");

    let (status, body) = c
        .call(
            Method::PATCH,
            &uri,
            Some(json!({"role": "clinician"})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let set = c.required_by(viewer).await.expect("promotion set it");
    assert_about_14_days_ahead(set);
    // The old token's role claim is ignored: the role is re-read per request.
    assert_eq!(c.me(&viewer_bearer).await["totp_enforcement"], "grace");

    let (status, body) = c
        .call(Method::PATCH, &uri, Some(json!({"role": "viewer"})), &owner)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.required_by(viewer).await, Some(set), "left in place");
    let me = c.me(&viewer_bearer).await;
    assert_eq!(me["totp_enforcement"], "not_required", "{me}");

    // Promoting again does not move it.
    let (status, body) = c
        .call(
            Method::PATCH,
            &uri,
            Some(json!({"role": "clinician"})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.required_by(viewer).await, Some(set));
    c.db.cleanup().await;
}

// ---- AUTH-13, AUTH-14: enforcement -------------------------------------------------------

#[tokio::test]
async fn a_passed_deadline_restricts_an_admin_to_enrolment() {
    let Some(c) = setup().await else { return };
    let (admin, bearer) = c.user(Role::Admin, "admin@example.org").await;
    let (status, _) = c.call(Method::GET, &c.sessions(), None, &bearer).await;
    assert_eq!(status, StatusCode::OK, "no deadline yet: grace");
    c.expire_deadline(admin).await;

    // Login still works and is audited.
    let (status, body) = c.login("admin@example.org").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = c.audit_after("auth.login_totp_enrolment_required").await;
    assert_eq!(rows.len(), 1);
    let stored = c.required_by(admin).await.unwrap();
    assert!(
        (ts(&rows[0]["totp_required_by"]) - stored)
            .num_milliseconds()
            .abs()
            < 1
    );
    let fresh = body["access_token"].as_str().unwrap().to_owned();

    for token in [&bearer, &fresh] {
        let (status, body) = c.call(Method::GET, &c.sessions(), None, token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(slug(&body), "totp-enrolment-required");
        let date = stored.format("%Y-%m-%d").to_string();
        assert!(body["detail"].as_str().unwrap().contains(&date), "{body}");
    }
    // Not exempt: the org list below the org, the profile edit, the users list.
    for (method, uri, body) in [
        (
            Method::PATCH,
            ME.to_owned(),
            Some(json!({"display_name": "A"})),
        ),
        (Method::GET, format!("/v1/orgs/{}/users", c.org), None),
        (Method::GET, format!("/v1/orgs/{}/audit", c.org), None),
    ] {
        let (status, resp) = c.call(method.clone(), &uri, body, &bearer).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}: {resp}");
        assert_eq!(slug(&resp), "totp-enrolment-required");
    }

    // The exempt routes answer as in phase 5.
    let me = c.me(&bearer).await;
    assert_eq!(me["totp_enforcement"], "required", "{me}");
    assert!(me["totp_required_by"].is_string());
    let (status, body) = c.call(Method::GET, &c.org_path(), None, &bearer).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let secret = c.enrol_and_confirm(&bearer).await;

    // The next request with the same token is unrestricted.
    let (status, body) = c.call(Method::GET, &c.sessions(), None, &bearer).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.me(&bearer).await["totp_enforcement"], "not_required");
    assert_eq!(c.required_by(admin).await, Some(stored), "not cleared");

    // AUTH-14: disabling is allowed and restricts again at once.
    let (status, body) = c
        .call(
            Method::DELETE,
            TOTP,
            Some(json!({"password": PASSWORD, "totp_code": code_for(&secret, 1)})),
            &bearer,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = c.call(Method::GET, &c.sessions(), None, &bearer).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(slug(&body), "totp-enrolment-required");
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_org_shell_stays_scoped_to_the_callers_org() {
    let Some(c) = setup().await else { return };
    let (admin, bearer) = c.user(Role::Admin, "admin@example.org").await;
    c.expire_deadline(admin).await;
    let other = seed_org(&c.db.pool, "Clinic B").await;
    let (status, body) = c
        .call(Method::GET, &format!("/v1/orgs/{other}"), None, &bearer)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    // An unknown route is a 404, never a pass through the exemption list.
    let (status, _) = c.call(Method::GET, "/v1/nope", None, &bearer).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_viewer_with_a_passed_deadline_is_unaffected() {
    let Some(c) = setup().await else { return };
    let (viewer, bearer) = c.user(Role::Viewer, "viewer@example.org").await;
    c.expire_deadline(viewer).await;
    let (status, body) = c.call(Method::GET, &c.sessions(), None, &bearer).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(c.me(&bearer).await["totp_enforcement"], "not_required");
    let (status, body) = c.login("viewer@example.org").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        audit_count(&c.db.pool, "auth.login_totp_enrolment_required").await,
        0
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_superadmin_behaves_like_an_admin() {
    let Some(c) = setup().await else { return };
    let (sa, bearer) = c.user(Role::Superadmin, "root@example.org").await;
    let me = c.me(&bearer).await;
    assert_eq!(me["totp_enforcement"], "grace", "{me}");
    assert!(me["org_id"].is_null());

    let (status, body) = c.login("root@example.org").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_about_14_days_ahead(c.required_by(sa).await.unwrap());

    c.expire_deadline(sa).await;
    assert_eq!(c.me(&bearer).await["totp_enforcement"], "required");
    let (status, body) = c.call(Method::GET, &c.org_path(), None, &bearer).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = c.call(Method::GET, &c.sessions(), None, &bearer).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(slug(&body), "totp-enrolment-required");
    c.db.cleanup().await;
}

#[tokio::test]
async fn device_plane_routes_are_unaffected() {
    let Some(c) = setup().await else { return };
    // A device token never goes through the user-plane policy, whatever the users' state.
    let (admin, _) = c.user(Role::Admin, "admin@example.org").await;
    c.expire_deadline(admin).await;
    let (_, device_token) = seed_device(&c.db.pool, c.org).await;
    let (status, body) = c
        .call(Method::GET, "/v1/config/bundle", None, &device_token)
        .await;
    assert_ne!(status, StatusCode::FORBIDDEN, "{body}");
    assert_ne!(slug(&body), "totp-enrolment-required");
    c.db.cleanup().await;
}

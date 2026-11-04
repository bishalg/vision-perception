//! owner: WP-L.1e
//!
//! Locale on users and organisations (I18N-12, I18N-13; acceptance 8) through the real
//! router: `GET/PATCH /v1/users/me`, `GET/PATCH /v1/orgs/{id}`, the audit rows, and every
//! row of the shared negotiation fixture. Needs `DATABASE_URL` (skipped otherwise).

mod common;

use asd_contract::api_v1::Role;
use asd_contract::i18n::{Locale, NegotiationCase};
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

struct Ctx {
    db: TestDb,
    state: asd_backend::app::AppState,
    app: Router,
    org: Uuid,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_with_pool(db.pool.clone());
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
        (id, access_token(&self.state, id, org, role))
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        bearer: &str,
        accept_language: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = match body {
            Some(b) => json(method, uri, &b, Some(bearer)),
            None => empty(method, uri, Some(bearer)),
        };
        if let Some(al) = accept_language {
            request
                .headers_mut()
                .insert(http::header::ACCEPT_LANGUAGE, al.parse().unwrap());
        }
        let (status, _, body) = send(self.app.clone(), request).await;
        (status, body)
    }

    fn org_uri(&self) -> String {
        format!("/v1/orgs/{}", self.org)
    }
}

async fn audit_row(pool: &PgPool, action: &str) -> (Value, Value) {
    sqlx::query_as::<_, (Option<Value>, Option<Value>)>(
        "SELECT before, after FROM audit_log WHERE action = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(action)
    .fetch_one(pool)
    .await
    .map(|(b, a)| (b.unwrap_or(Value::Null), a.unwrap_or(Value::Null)))
    .unwrap_or_else(|e| panic!("no audit row {action}: {e}"))
}

#[tokio::test]
async fn me_defaults_to_en_with_null_locale() {
    let Some(c) = setup().await else { return };
    let (_, tok) = c.user(Role::Clinician, "c@example.org").await;
    let (s, body) = c.call(Method::GET, "/v1/users/me", None, &tok, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["locale"], Value::Null);
    assert_eq!(body["effective_locale"], "en");
}

#[tokio::test]
async fn org_default_flows_to_users_and_user_override_wins() {
    let Some(c) = setup().await else { return };
    let (_, admin) = c.user(Role::Admin, "a@example.org").await;
    let (_, clin) = c.user(Role::Clinician, "c@example.org").await;

    let (s, org) = c.call(Method::GET, &c.org_uri(), None, &admin, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(org["default_locale"], "en");

    // A clinician may not set it.
    let ne = Some(json!({"default_locale": "ne"}));
    let (s, _) = c
        .call(Method::PATCH, &c.org_uri(), ne.clone(), &clin, None)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // An admin may; the audit diff names the field.
    let (s, org) = c.call(Method::PATCH, &c.org_uri(), ne, &admin, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(org["default_locale"], "ne");
    let (before, after) = audit_row(&c.db.pool, "org.update").await;
    assert_eq!(before["default_locale"], "en");
    assert_eq!(after["default_locale"], "ne");
    let (_, org) = c.call(Method::GET, &c.org_uri(), None, &clin, None).await;
    assert_eq!(org["default_locale"], "ne");

    // The clinician now follows the clinic, whatever Accept-Language says.
    let (_, me) = c
        .call(Method::GET, "/v1/users/me", None, &clin, Some("en"))
        .await;
    assert_eq!(me["locale"], Value::Null);
    assert_eq!(me["effective_locale"], "ne");

    // Own override.
    let (s, me) = c
        .call(
            Method::PATCH,
            "/v1/users/me",
            Some(json!({"locale": "en"})),
            &clin,
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["locale"], "en");
    assert_eq!(me["effective_locale"], "en");
    let (before, after) = audit_row(&c.db.pool, "user.profile_update").await;
    assert_eq!(before["locale"], Value::Null);
    assert_eq!(after["locale"], "en");
    let (_, me) = c.call(Method::GET, "/v1/users/me", None, &clin, None).await;
    assert_eq!(me["locale"], "en");

    // null clears it: back to the clinic.
    let (s, me) = c
        .call(
            Method::PATCH,
            "/v1/users/me",
            Some(json!({"locale": null})),
            &clin,
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["locale"], Value::Null);
    assert_eq!(me["effective_locale"], "ne");
    let (before, after) = audit_row(&c.db.pool, "user.profile_update").await;
    assert_eq!(before["locale"], "en");
    assert_eq!(after["locale"], Value::Null);

    // An unrelated profile edit leaves the locale alone.
    c.call(
        Method::PATCH,
        "/v1/users/me",
        Some(json!({"locale": "en"})),
        &clin,
        None,
    )
    .await;
    let (_, me) = c
        .call(
            Method::PATCH,
            "/v1/users/me",
            Some(json!({"display_name": "Dr A"})),
            &clin,
            None,
        )
        .await;
    assert_eq!(me["locale"], "en");
}

#[tokio::test]
async fn unchanged_locale_writes_no_audit_row() {
    let Some(c) = setup().await else { return };
    let (_, clin) = c.user(Role::Clinician, "c@example.org").await;
    let (_, admin) = c.user(Role::Admin, "a@example.org").await;
    let (s, _) = c
        .call(
            Method::PATCH,
            "/v1/users/me",
            Some(json!({"locale": null})),
            &clin,
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = c
        .call(
            Method::PATCH,
            &c.org_uri(),
            Some(json!({"default_locale": "en"})),
            &admin,
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(audit_count(&c.db.pool, "user.profile_update").await, 0);
    assert_eq!(audit_count(&c.db.pool, "org.update").await, 0);
}

#[tokio::test]
async fn unsupported_locales_are_validation_failed() {
    let Some(c) = setup().await else { return };
    let (_, clin) = c.user(Role::Clinician, "c@example.org").await;
    let (_, admin) = c.user(Role::Admin, "a@example.org").await;
    let (s, body) = c
        .call(
            Method::PATCH,
            "/v1/users/me",
            Some(json!({"locale": "fr"})),
            &clin,
            None,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(slug(&body), "validation-failed");
    let (s, body) = c
        .call(
            Method::PATCH,
            &c.org_uri(),
            Some(json!({"default_locale": "hi"})),
            &admin,
            None,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(slug(&body), "validation-failed");
}

#[tokio::test]
async fn settings_locale_points_at_default_locale() {
    let Some(c) = setup().await else { return };
    let (_, admin) = c.user(Role::Admin, "a@example.org").await;
    let (s, body) = c
        .call(
            Method::PATCH,
            &c.org_uri(),
            Some(json!({"settings": {"locale": "ne"}})),
            &admin,
            None,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(slug(&body), "validation-failed");
    assert!(body.to_string().contains("default_locale"), "{body}");
}

#[tokio::test]
async fn superadmin_without_org_uses_accept_language() {
    let Some(c) = setup().await else { return };
    let (_, tok) = c.user(Role::Superadmin, "s@example.org").await;
    let (s, me) = c
        .call(Method::GET, "/v1/users/me", None, &tok, Some("ne-NP"))
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["locale"], Value::Null);
    assert_eq!(me["effective_locale"], "ne");
    let (_, me) = c.call(Method::GET, "/v1/users/me", None, &tok, None).await;
    assert_eq!(me["effective_locale"], "en");
}

/// Every row of the shared fixture through `GET /v1/users/me`: a user with an org carries
/// the row's org default (a row with no org is a superadmin without a home org).
#[tokio::test]
async fn shared_negotiation_fixture_through_http() {
    let Some(c) = setup().await else { return };
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/spec/schemas/asd/i18n-negotiation-cases.json"
    );
    let cases: Vec<NegotiationCase> =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture file"))
            .expect("fixture parses");
    assert!(cases.len() >= 25);
    for (i, case) in cases.iter().enumerate() {
        let (role, org) = match case.org {
            Some(l) => {
                let org = seed_org(&c.db.pool, &format!("Fixture {i}")).await;
                sqlx::query("UPDATE organizations SET default_locale = $2 WHERE id = $1")
                    .bind(org)
                    .bind(l.as_str())
                    .execute(&c.db.pool)
                    .await
                    .unwrap();
                (Role::Clinician, Some(org))
            }
            None => (Role::Superadmin, None),
        };
        let id = seed_user(&c.db.pool, org, role, &format!("f{i}@example.org")).await;
        sqlx::query("UPDATE users SET locale = $2 WHERE id = $1")
            .bind(id)
            .bind(case.user.map(Locale::as_str))
            .execute(&c.db.pool)
            .await
            .unwrap();
        let tok = access_token(&c.state, id, org, role);
        let (s, me) = c
            .call(
                Method::GET,
                "/v1/users/me",
                None,
                &tok,
                case.accept_language.as_deref(),
            )
            .await;
        assert_eq!(s, StatusCode::OK, "row {i}");
        assert_eq!(
            me["effective_locale"],
            case.expected.as_str(),
            "row {i}: {case:?}"
        );
    }
}

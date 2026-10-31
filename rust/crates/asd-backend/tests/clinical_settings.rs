//! owner: WP-C.9
//!
//! Org settings for the clinician workspace (NOTE-7 `timezone`, `paper`,
//! `letterhead.address_lines`; MRN-2 `identity.patient_ref_mode`; CW-2
//! `clinical.worklist_scope`; SIGN-6 `clinical.cosign_policy`): validated keys and values,
//! nested merge, and an audit row that carries only what changed (G3, trap 2). Plus the
//! signer profile (NOTE-6): `display_name` and `professional_registration` on user create,
//! update and `GET/PATCH /v1/users/me`. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::app::AppState;
use asd_contract::api_v1::Role;
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

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
    let org = seed_org(&db.pool, "Clinic A").await;
    Some(Ctx {
        db,
        state,
        app,
        org,
    })
}

impl Ctx {
    async fn token(&self, org: Option<Uuid>, role: Role) -> (Uuid, String) {
        let email = format!("{}-{}@example.org", role_str(role), Uuid::new_v4().simple());
        let id = seed_user(&self.db.pool, org, role, &email).await;
        (id, access_token(&self.state, id, org, role))
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        bearer: &str,
    ) -> (StatusCode, Value) {
        let req = match body {
            Some(b) => json(method, uri, &b, Some(bearer)),
            None => empty(method, uri, Some(bearer)),
        };
        let (s, _, b) = send(self.app.clone(), req).await;
        (s, b)
    }

    async fn patch_settings(&self, bearer: &str, settings: Value) -> (StatusCode, Value) {
        self.call(
            Method::PATCH,
            &format!("/v1/orgs/{}", self.org),
            Some(json!({"settings": settings})),
            bearer,
        )
        .await
    }

    async fn settings(&self) -> Value {
        sqlx::query_scalar("SELECT settings FROM organizations WHERE id = $1")
            .bind(self.org)
            .fetch_one(&self.db.pool)
            .await
            .unwrap()
    }

    async fn last_org_audit(&self) -> (Value, Value) {
        let (before, after): (Option<Value>, Option<Value>) = sqlx::query_as(
            "SELECT before, after FROM audit_log WHERE action = 'org.update' ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&self.db.pool)
        .await
        .unwrap();
        (before.unwrap(), after.unwrap())
    }
}

fn param_names(body: &Value) -> Vec<String> {
    body["invalid_params"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| p["name"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn valid_clinical_settings_are_stored_and_merged() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    let (status, body) = c
        .patch_settings(
            &owner,
            json!({
                "timezone": "Asia/Kathmandu",
                "paper": "letter",
                "letterhead": {"address_lines": ["Child Development Clinic", "Maharajgunj, Kathmandu"]},
                "identity": {"patient_ref_mode": "mrn"},
                "clinical": {"worklist_scope": "flagged", "cosign_policy": "red_flag"},
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Nested objects merge: changing one key keeps its siblings.
    let (status, body) = c
        .patch_settings(&owner, json!({"clinical": {"cosign_policy": "always"}}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let s = c.settings().await;
    assert_eq!(
        s["clinical"],
        json!({"worklist_scope": "flagged", "cosign_policy": "always"})
    );
    assert_eq!(s["timezone"], "Asia/Kathmandu");
    assert_eq!(s["identity"]["patient_ref_mode"], "mrn");
    // `null` removes a key (RFC 7396 merge patch), falling back to the default.
    let (status, _) = c.patch_settings(&owner, json!({"paper": null})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(c.settings().await.get("paper").is_none());
    for tz in [
        "UTC",
        "Asia/Kolkata",
        "America/Argentina/Buenos_Aires",
        "Etc/GMT+5",
    ] {
        let (status, body) = c.patch_settings(&owner, json!({"timezone": tz})).await;
        assert_eq!(status, StatusCode::OK, "{tz}: {body}");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn invalid_clinical_settings_name_the_key() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    for (settings, name) in [
        (json!({"timezone": "Kathmandu time"}), "settings.timezone"),
        (json!({"timezone": 545}), "settings.timezone"),
        (json!({"timezone": "../../etc/passwd"}), "settings.timezone"),
        (json!({"paper": "a5"}), "settings.paper"),
        (json!({"letterhead": "x"}), "settings.letterhead"),
        (
            json!({"letterhead": {"logo": "x"}}),
            "settings.letterhead.logo",
        ),
        (
            json!({"letterhead": {"address_lines": ["1", "2", "3", "4", "5"]}}),
            "settings.letterhead.address_lines",
        ),
        (
            json!({"letterhead": {"address_lines": ["x".repeat(81)]}}),
            "settings.letterhead.address_lines",
        ),
        (
            json!({"letterhead": {"address_lines": [1]}}),
            "settings.letterhead.address_lines",
        ),
        (
            json!({"letterhead": {"address_lines": ["line\nbreak"]}}),
            "settings.letterhead.address_lines",
        ),
        (
            json!({"identity": {"patient_ref_mode": "name"}}),
            "settings.identity.patient_ref_mode",
        ),
        (json!({"identity": {"mrn": "123"}}), "settings.identity.mrn"),
        (
            json!({"clinical": {"worklist_scope": "some"}}),
            "settings.clinical.worklist_scope",
        ),
        (
            json!({"clinical": {"cosign_policy": "yellow_flag"}}),
            "settings.clinical.cosign_policy",
        ),
        (
            json!({"clinical": {"retention": 3}}),
            "settings.clinical.retention",
        ),
        // Secrets, MRNs and integration settings never go into org settings (G3, INTG-1).
        (
            json!({"webhook_secret": "whsec_x"}),
            "settings.webhook_secret",
        ),
        (json!({"sftp_password": "x"}), "settings.sftp_password"),
        (json!({"api_token": "x"}), "settings.api_token"),
        (json!({"mrn": "123"}), "settings.mrn"),
        (json!({"integrations": {}}), "settings.integrations"),
    ] {
        let (status, body) = c.patch_settings(&owner, settings.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{settings}: {body}");
        assert!(
            param_names(&body).contains(&name.to_owned()),
            "{settings}: {body}"
        );
    }
    assert_eq!(c.settings().await, json!({}), "nothing was stored");
    c.db.cleanup().await;
}

#[tokio::test]
async fn audit_row_carries_only_the_changed_settings() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    let (status, _) = c
        .patch_settings(
            &owner,
            json!({"letterhead": {"address_lines": ["Clinic Road 1"]}, "audit_rate": 0.1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = c.patch_settings(&owner, json!({"paper": "letter"})).await;
    assert_eq!(status, StatusCode::OK);
    let (before, after) = c.last_org_audit().await;
    assert_eq!(before, json!({"settings": {"paper": null}}), "{before}");
    assert_eq!(after, json!({"settings": {"paper": "letter"}}), "{after}");
    assert!(
        !after.to_string().contains("Clinic Road"),
        "unchanged keys stay out"
    );

    // Name and retention appear only when they change.
    let (status, _) = c
        .call(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            Some(json!({"retention_months": 12, "name": "Clinic A"})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (before, after) = c.last_org_audit().await;
    assert_eq!(before, json!({"retention_months": 24}));
    assert_eq!(after, json!({"retention_months": 12}));
    c.db.cleanup().await;
}

// ---- signer profile (NOTE-6) ------------------------------------------------------------

#[tokio::test]
async fn profile_on_create_update_and_list() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    let users = format!("/v1/orgs/{}/users", c.org);
    let (status, created) = c
        .call(
            Method::POST,
            &users,
            Some(json!({
                "email": "dr.a@example.org",
                "role": "clinician",
                "password": "a long enough password",
                "display_name": "  Dr. Anita Sharma ",
                "professional_registration": "NMC 12345",
            })),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["display_name"], "Dr. Anita Sharma");
    assert_eq!(created["professional_registration"], "NMC 12345");
    let id = created["id"].as_str().unwrap().to_owned();

    let (status, updated) = c
        .call(
            Method::PATCH,
            &format!("{users}/{id}"),
            Some(json!({"professional_registration": "NMC 54321"})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["professional_registration"], "NMC 54321");
    assert_eq!(updated["display_name"], "Dr. Anita Sharma");
    let (after,): (Option<Value>,) = sqlx::query_as(
        "SELECT after FROM audit_log WHERE action = 'user.update' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(after.unwrap()["professional_registration"], "NMC 54321");

    // An empty string clears.
    let (status, cleared) = c
        .call(
            Method::PATCH,
            &format!("{users}/{id}"),
            Some(json!({"professional_registration": ""})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["professional_registration"].is_null());

    let (status, page) = c.call(Method::GET, &users, None, &owner).await;
    assert_eq!(status, StatusCode::OK);
    let item = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == id.as_str())
        .unwrap()
        .clone();
    assert_eq!(item["display_name"], "Dr. Anita Sharma");
    c.db.cleanup().await;
}

#[tokio::test]
async fn profile_lengths_and_characters_are_validated() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    let (target, _) = c.token(Some(c.org), Role::Clinician).await;
    let uri = format!("/v1/orgs/{}/users/{target}", c.org);
    for (body, name) in [
        (json!({"display_name": "a".repeat(121)}), "display_name"),
        (
            json!({"professional_registration": "a".repeat(61)}),
            "professional_registration",
        ),
        (json!({"display_name": "Dr\nX"}), "display_name"),
    ] {
        let (status, resp) = c
            .call(Method::PATCH, &uri, Some(body.clone()), &owner)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {resp}");
        assert!(param_names(&resp).contains(&name.to_owned()), "{resp}");
    }
    // 120 and 60 characters (not bytes) are fine.
    let (status, resp) = c
        .call(
            Method::PATCH,
            &uri,
            Some(json!({"display_name": "न".repeat(120), "professional_registration": "न".repeat(60)})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn users_edit_their_own_profile() {
    let Some(c) = setup().await else { return };
    let (id, clinician) = c.token(Some(c.org), Role::Clinician).await;
    let (status, me) = c.call(Method::GET, "/v1/users/me", None, &clinician).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["id"], id.to_string());
    assert!(me["display_name"].is_null());
    assert_eq!(me["role"], "clinician");

    let (status, me) = c
        .call(
            Method::PATCH,
            "/v1/users/me",
            Some(json!({"display_name": "Dr. B", "professional_registration": "NMC 1"})),
            &clinician,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["display_name"], "Dr. B");
    let (status, me) = c.call(Method::GET, "/v1/users/me", None, &clinician).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["professional_registration"], "NMC 1");
    let (action, actor): (String, Option<String>) =
        sqlx::query_as("SELECT action, actor_id FROM audit_log ORDER BY id DESC LIMIT 1")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(
        (action.as_str(), actor),
        ("user.profile_update", Some(id.to_string()))
    );

    // The self route changes only the profile: a role or status in the body is rejected.
    let (status, _) = c
        .call(
            Method::PATCH,
            "/v1/users/me",
            Some(json!({"role": "owner"})),
            &clinician,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Superadmin (no org) has a profile too.
    let (_, root) = c.token(None, Role::Superadmin).await;
    let (status, me) = c.call(Method::GET, "/v1/users/me", None, &root).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert!(me["org_id"].is_null());

    // No bearer: 401.
    let (status, _, _) = send(c.app.clone(), empty(Method::GET, "/v1/users/me", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    c.db.cleanup().await;
}

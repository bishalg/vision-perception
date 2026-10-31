//! owner: WP-B.2
//!
//! AUTH-4: the RBAC matrix over HTTP, table-driven (7 roles x the B.2 routes): allowed
//! roles get 2xx, other roles of the same org 403, every role of another org 404, no
//! bearer 401; `superadmin` reaches every org. Plus the user/org management rules that
//! sit on top of the matrix (role assignment ceilings, self-protection, last owner).
//! Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::app::AppState;
use asd_backend::auth::rbac::{self, Action};
use asd_contract::api_v1::Role;
use axum::Router;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

const ORG_ROLES: [Role; 6] = [
    Role::Owner,
    Role::Admin,
    Role::Clinician,
    Role::Researcher,
    Role::Reviewer,
    Role::Viewer,
];

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
    org: Uuid,
    other_org: Uuid,
}

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let other_org = seed_org(&db.pool, "Clinic B").await;
    Some(Ctx {
        db,
        state,
        app,
        org,
        other_org,
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
}

/// One route of the matrix. `build` makes a fresh request target in `org` (so mutating
/// calls do not depend on each other).
struct Route {
    name: &'static str,
    allowed: &'static [Role],
    success: StatusCode,
}

const MANAGERS: &[Role] = &[Role::Owner, Role::Admin];
const EVERYONE: &[Role] = &ORG_ROLES;
const DEVICE_STAFF: &[Role] = &[Role::Owner, Role::Admin, Role::Clinician];

const ROUTES: &[Route] = &[
    Route {
        name: "GET /orgs/{id}",
        allowed: EVERYONE,
        success: StatusCode::OK,
    },
    Route {
        name: "PATCH /orgs/{id}",
        allowed: MANAGERS,
        success: StatusCode::OK,
    },
    Route {
        name: "GET /orgs/{id}/users",
        allowed: MANAGERS,
        success: StatusCode::OK,
    },
    Route {
        name: "POST /orgs/{id}/users",
        allowed: MANAGERS,
        success: StatusCode::CREATED,
    },
    Route {
        name: "PATCH /orgs/{id}/users/{user_id}",
        allowed: MANAGERS,
        success: StatusCode::OK,
    },
    Route {
        name: "GET /orgs/{id}/devices",
        allowed: DEVICE_STAFF,
        success: StatusCode::OK,
    },
    Route {
        name: "POST /orgs/{id}/devices/enrol-code",
        allowed: DEVICE_STAFF,
        success: StatusCode::CREATED,
    },
    Route {
        name: "POST /devices/{id}/revoke",
        allowed: MANAGERS,
        success: StatusCode::OK,
    },
    Route {
        name: "GET /orgs/{id}/audit",
        allowed: MANAGERS,
        success: StatusCode::OK,
    },
];

/// Builds the concrete request for `route` against resources of `org`.
async fn request_for(c: &Ctx, route: &Route, org: Uuid) -> (Method, String, Option<Value>) {
    match route.name {
        "GET /orgs/{id}" => (Method::GET, format!("/v1/orgs/{org}"), None),
        "PATCH /orgs/{id}" => (
            Method::PATCH,
            format!("/v1/orgs/{org}"),
            Some(json!({"retention_months": 36})),
        ),
        "GET /orgs/{id}/users" => (Method::GET, format!("/v1/orgs/{org}/users"), None),
        "POST /orgs/{id}/users" => (
            Method::POST,
            format!("/v1/orgs/{org}/users"),
            Some(json!({
                "email": format!("new-{}@example.org", Uuid::new_v4().simple()),
                "role": "viewer",
                "password": "a long enough password"
            })),
        ),
        "PATCH /orgs/{id}/users/{user_id}" => {
            let email = format!("target-{}@example.org", Uuid::new_v4().simple());
            let target = seed_user(&c.db.pool, Some(org), Role::Viewer, &email).await;
            (
                Method::PATCH,
                format!("/v1/orgs/{org}/users/{target}"),
                Some(json!({"role": "reviewer"})),
            )
        }
        "GET /orgs/{id}/devices" => (Method::GET, format!("/v1/orgs/{org}/devices"), None),
        "POST /orgs/{id}/devices/enrol-code" => (
            Method::POST,
            format!("/v1/orgs/{org}/devices/enrol-code"),
            Some(json!({})),
        ),
        "POST /devices/{id}/revoke" => {
            let (device, _) = seed_device(&c.db.pool, org).await;
            (Method::POST, format!("/v1/devices/{device}/revoke"), None)
        }
        "GET /orgs/{id}/audit" => (Method::GET, format!("/v1/orgs/{org}/audit"), None),
        other => panic!("unknown route {other}"),
    }
}

#[tokio::test]
async fn matrix_same_org_allowed_2xx_denied_403() {
    let Some(c) = setup().await else { return };
    for role in ORG_ROLES {
        let (_, token) = c.token(Some(c.org), role).await;
        for route in ROUTES {
            let (method, uri, body) = request_for(&c, route, c.org).await;
            let (status, resp) = c.call(method, &uri, body, &token).await;
            let want = if route.allowed.contains(&role) {
                route.success
            } else {
                StatusCode::FORBIDDEN
            };
            assert_eq!(status, want, "{role:?} {}: {resp}", route.name);
            if want == StatusCode::FORBIDDEN {
                assert_eq!(slug(&resp), "forbidden");
            }
        }
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn matrix_other_org_is_404_for_every_role() {
    let Some(c) = setup().await else { return };
    for role in ORG_ROLES {
        let (_, token) = c.token(Some(c.org), role).await;
        for route in ROUTES {
            let (method, uri, body) = request_for(&c, route, c.other_org).await;
            let (status, resp) = c.call(method, &uri, body, &token).await;
            // Org-path routes answer 404 before the role check; resource routes check
            // the role first (a 403 that does not depend on the resource), then 404.
            let resource_route = route.name.starts_with("POST /devices/");
            let want = if resource_route && !route.allowed.contains(&role) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::NOT_FOUND
            };
            assert_eq!(
                status, want,
                "{role:?} {} on another org: {resp}",
                route.name
            );
        }
        // A nonexistent org is indistinguishable from someone else's.
        let (status, _) = c
            .call(
                Method::GET,
                &format!("/v1/orgs/{}", Uuid::new_v4()),
                None,
                &token,
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn matrix_superadmin_reaches_every_org() {
    let Some(c) = setup().await else { return };
    let (_, token) = c.token(None, Role::Superadmin).await;
    for org in [c.org, c.other_org] {
        for route in ROUTES {
            let (method, uri, body) = request_for(&c, route, org).await;
            let (status, resp) = c.call(method, &uri, body, &token).await;
            assert_eq!(status, route.success, "superadmin {}: {resp}", route.name);
        }
    }
    c.db.cleanup().await;
}

#[test]
fn matrix_table_covers_every_action_and_superadmin_has_all_but_clinical_content() {
    // No database: the single table in auth/rbac.rs. Superadmin has every action except
    // the clinical-content carve-out (RBAC-3, WP-C.9; covered in rbac_clinical.rs).
    for action in Action::ALL {
        assert_eq!(
            rbac::allows(Role::Superadmin, *action),
            !rbac::CLINICAL_CONTENT_ACTIONS.contains(action),
            "{action:?}"
        );
        assert!(
            rbac::MATRIX.iter().any(|(a, _)| a == action),
            "{action:?} has no row in MATRIX"
        );
    }
    for role in ORG_ROLES {
        assert!(rbac::allows(role, Action::OrgRead), "{role:?}");
        assert!(rbac::allows(role, Action::SessionRead), "{role:?}");
    }
    // Viewers only read.
    for action in Action::ALL {
        if rbac::allows(Role::Viewer, *action) {
            assert!(action.is_read_only(), "viewer may {action:?}");
        }
    }
}

// ---- user and org management rules on top of the matrix -------------------------------

#[tokio::test]
async fn role_assignment_never_exceeds_the_actors_rank() {
    let Some(c) = setup().await else { return };
    let (_, admin) = c.token(Some(c.org), Role::Admin).await;
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    let users = format!("/v1/orgs/{}/users", c.org);
    let body = |role: &str| {
        json!({
            "email": format!("{role}-{}@example.org", Uuid::new_v4().simple()),
            "role": role,
            "password": "a long enough password"
        })
    };

    let (status, resp) = c
        .call(Method::POST, &users, Some(body("owner")), &admin)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "admin creating an owner: {resp}"
    );
    let (status, resp) = c
        .call(Method::POST, &users, Some(body("admin")), &admin)
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "admin creating an admin: {resp}"
    );
    let (status, resp) = c
        .call(Method::POST, &users, Some(body("owner")), &owner)
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "owner creating an owner: {resp}"
    );
    assert_eq!(resp["role"], "owner");
    assert_eq!(resp["status"], "active");
    assert_eq!(resp["org_id"], c.org.to_string());
    assert!(resp.get("pw_hash").is_none() && resp.get("password").is_none());

    let (_, root) = c.token(None, Role::Superadmin).await;
    for actor in [&owner, &root] {
        let (status, resp) = c
            .call(Method::POST, &users, Some(body("superadmin")), actor)
            .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "superadmin is not an org role: {resp}"
        );
    }

    // An admin may not touch an owner, nor promote anyone to owner.
    let target_owner = seed_user(&c.db.pool, Some(c.org), Role::Owner, "o2@example.org").await;
    let (status, _) = c
        .call(
            Method::PATCH,
            &format!("{users}/{target_owner}"),
            Some(json!({"status": "disabled"})),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let viewer = seed_user(&c.db.pool, Some(c.org), Role::Viewer, "v@example.org").await;
    let (status, _) = c
        .call(
            Method::PATCH,
            &format!("{users}/{viewer}"),
            Some(json!({"role": "owner"})),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    c.db.cleanup().await;
}

/// RBAC-3 / ADR-0008 D2 (security audit finding 1): an actor that may not read clinical
/// content (superadmin, admin) never creates a user, nor promotes one, into a role holding
/// clinical-content actions it lacks: it would choose (or already know) that user's password
/// and log in as them. Owners still provision clinicians.
#[tokio::test]
async fn non_clinical_actors_never_provision_clinical_roles() {
    let Some(c) = setup().await else { return };
    let (_, root) = c.token(None, Role::Superadmin).await;
    let (_, admin) = c.token(Some(c.org), Role::Admin).await;
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    let users = format!("/v1/orgs/{}/users", c.org);
    let body = |role: &str| {
        json!({
            "email": format!("{role}-{}@example.org", Uuid::new_v4().simple()),
            "role": role,
            "password": "a long enough password"
        })
    };
    let refused = |status: StatusCode, resp: &Value, what: &str| {
        assert_eq!(status, StatusCode::FORBIDDEN, "{what}: {resp}");
        assert_eq!(slug(resp), "clinical-role-not-assignable", "{what}: {resp}");
    };

    // Superadmin: no owner, clinician, admin (MRN read) or researcher (de-identified export).
    for role in ["owner", "clinician", "admin", "researcher"] {
        let (status, resp) = c.call(Method::POST, &users, Some(body(role)), &root).await;
        refused(status, &resp, &format!("superadmin creating {role}"));
    }
    // Admin: no clinician (owner is already above its rank: plain 403).
    let (status, resp) = c
        .call(Method::POST, &users, Some(body("clinician")), &admin)
        .await;
    refused(status, &resp, "admin creating a clinician");
    // Roles without clinical-content actions stay provisionable.
    for (actor, role) in [
        (&root, "viewer"),
        (&root, "reviewer"),
        (&admin, "researcher"),
    ] {
        let (status, resp) = c.call(Method::POST, &users, Some(body(role)), actor).await;
        assert_eq!(status, StatusCode::CREATED, "creating {role}: {resp}");
    }
    // Nothing was created by the refused calls.
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE org_id = $1 AND role = 'clinician'")
            .bind(c.org)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(n, 0);

    // Promotions: a viewer (whose password the actor may have chosen) never becomes a
    // clinician or an owner through a non-clinical actor.
    let viewer = seed_user(&c.db.pool, Some(c.org), Role::Viewer, "pv@example.org").await;
    for (actor, role, who) in [
        (&root, "clinician", "superadmin"),
        (&root, "owner", "superadmin"),
        (&root, "admin", "superadmin"),
        (&admin, "clinician", "admin"),
    ] {
        let (status, resp) = c
            .call(
                Method::PATCH,
                &format!("{users}/{viewer}"),
                Some(json!({"role": role})),
                actor,
            )
            .await;
        refused(
            status,
            &resp,
            &format!("{who} promoting a viewer to {role}"),
        );
    }
    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
        .bind(viewer)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(role, "viewer");

    // An owner creates and promotes clinicians.
    let (status, resp) = c
        .call(Method::POST, &users, Some(body("clinician")), &owner)
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "owner creating a clinician: {resp}"
    );
    assert_eq!(resp["role"], "clinician");
    let (status, resp) = c
        .call(
            Method::PATCH,
            &format!("{users}/{viewer}"),
            Some(json!({"role": "clinician"})),
            &owner,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "owner promoting to clinician: {resp}"
    );
    // Demoting, or changing only the status of, a clinical user stays an admin's job.
    let (status, resp) = c
        .call(
            Method::PATCH,
            &format!("{users}/{viewer}"),
            Some(json!({"status": "disabled"})),
            &admin,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "admin disabling a clinician: {resp}"
    );
    let (status, resp) = c
        .call(
            Method::PATCH,
            &format!("{users}/{viewer}"),
            Some(json!({"role": "viewer"})),
            &root,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "superadmin demoting a clinician: {resp}"
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn users_cannot_change_their_own_role_or_status_and_the_last_owner_stays() {
    let Some(c) = setup().await else { return };
    let (owner_id, owner) = c.token(Some(c.org), Role::Owner).await;
    let users = format!("/v1/orgs/{}/users", c.org);
    for patch in [json!({"role": "viewer"}), json!({"status": "disabled"})] {
        let (status, resp) = c
            .call(
                Method::PATCH,
                &format!("{users}/{owner_id}"),
                Some(patch),
                &owner,
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{resp}");
    }
    let (_, root) = c.token(None, Role::Superadmin).await;
    for patch in [json!({"role": "admin"}), json!({"status": "disabled"})] {
        let (status, resp) = c
            .call(
                Method::PATCH,
                &format!("{users}/{owner_id}"),
                Some(patch),
                &root,
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{resp}");
        assert_eq!(slug(&resp), "last-owner");
    }
    // With a second owner the first can be demoted.
    seed_user(&c.db.pool, Some(c.org), Role::Owner, "o2@example.org").await;
    let (status, resp) = c
        .call(
            Method::PATCH,
            &format!("{users}/{owner_id}"),
            Some(json!({"role": "admin"})),
            &root,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["role"], "admin");
    c.db.cleanup().await;
}

#[tokio::test]
async fn disabling_a_user_revokes_their_refresh_tokens_and_access() {
    let Some(c) = setup().await else { return };
    let (_, admin) = c.token(Some(c.org), Role::Admin).await;
    let email = "victim@example.org";
    let victim = seed_user(&c.db.pool, Some(c.org), Role::Clinician, email).await;
    let (status, _, pair) = send(
        c.app.clone(),
        json(
            Method::POST,
            "/v1/auth/login",
            &json!({"email": email, "password": PASSWORD}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, resp) = c
        .call(
            Method::PATCH,
            &format!("/v1/orgs/{}/users/{victim}", c.org),
            Some(json!({"status": "disabled"})),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["status"], "disabled");
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(victim)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(live, 0);
    let (status, _, _) = send(
        c.app.clone(),
        empty(
            Method::GET,
            &format!("/v1/orgs/{}", c.org),
            pair["access_token"].as_str(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    c.db.cleanup().await;
}

#[tokio::test]
async fn user_creation_validates_and_rejects_duplicate_emails() {
    let Some(c) = setup().await else { return };
    let (_, admin) = c.token(Some(c.org), Role::Admin).await;
    let users = format!("/v1/orgs/{}/users", c.org);
    let ok =
        json!({"email": "Dup@Example.org", "role": "viewer", "password": "a long enough password"});
    let (status, resp) = c.call(Method::POST, &users, Some(ok.clone()), &admin).await;
    assert_eq!(status, StatusCode::CREATED, "{resp}");
    assert_eq!(resp["email"], "dup@example.org");

    let mut dup = ok.clone();
    dup["email"] = json!("dup@EXAMPLE.org");
    let (status, resp) = c.call(Method::POST, &users, Some(dup), &admin).await;
    assert_eq!(status, StatusCode::CONFLICT, "{resp}");
    assert_eq!(slug(&resp), "email-taken");

    for bad in [
        json!({"email": "not-an-email", "role": "viewer", "password": "a long enough password"}),
        json!({"email": "short@example.org", "role": "viewer", "password": "short"}),
        json!({"email": "x@example.org", "role": "wizard", "password": "a long enough password"}),
    ] {
        let (status, resp) = c
            .call(Method::POST, &users, Some(bad.clone()), &admin)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {resp}");
        assert_eq!(slug(&resp), "validation-failed");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn user_list_is_org_scoped_and_paginated() {
    let Some(c) = setup().await else { return };
    let (_, admin) = c.token(Some(c.org), Role::Admin).await;
    for i in 0..3 {
        seed_user(
            &c.db.pool,
            Some(c.org),
            Role::Viewer,
            &format!("u{i}@example.org"),
        )
        .await;
    }
    seed_user(
        &c.db.pool,
        Some(c.other_org),
        Role::Viewer,
        "elsewhere@example.org",
    )
    .await;

    let mut seen = Vec::new();
    let mut uri = format!("/v1/orgs/{}/users?limit=3", c.org);
    loop {
        let (status, page) = c.call(Method::GET, &uri, None, &admin).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        for u in page["items"].as_array().unwrap() {
            assert_eq!(u["org_id"], c.org.to_string());
            seen.push(u["email"].as_str().unwrap().to_owned());
        }
        match page["next_cursor"].as_str() {
            Some(cursor) => uri = format!("/v1/orgs/{}/users?limit=3&cursor={cursor}", c.org),
            None => break,
        }
    }
    assert_eq!(seen.len(), 4, "admin + 3 viewers: {seen:?}");
    assert!(!seen.contains(&"elsewhere@example.org".to_owned()));
    c.db.cleanup().await;
}

#[tokio::test]
async fn org_patch_merges_settings_and_validates() {
    let Some(c) = setup().await else { return };
    let (_, owner) = c.token(Some(c.org), Role::Owner).await;
    let uri = format!("/v1/orgs/{}", c.org);
    let (status, resp) = c
        .call(
            Method::PATCH,
            &uri,
            Some(json!({"name": "Clinic A North", "settings": {"audit_rate": 0.1}})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["name"], "Clinic A North");
    assert_eq!(resp["retention_months"], 24);
    let (status, _) = c
        .call(
            Method::PATCH,
            &uri,
            Some(json!({"settings": {"locale": "en-GB"}})),
            &owner,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let settings: Value = sqlx::query_scalar("SELECT settings FROM organizations WHERE id = $1")
        .bind(c.org)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(
        settings,
        json!({"audit_rate": 0.1, "locale": "en-GB"}),
        "merged"
    );

    for bad in [
        json!({"name": ""}),
        json!({"retention_months": 0}),
        json!({"retention_months": 241}),
        json!({"settings": {"audit_rate": 1.5}}),
        json!({"settings": {"audit_rate": "high"}}),
    ] {
        let (status, resp) = c.call(Method::PATCH, &uri, Some(bad.clone()), &owner).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {resp}");
    }
    c.db.cleanup().await;
}

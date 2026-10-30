//! owner: WP-B.2
//!
//! AUTH-1 (Argon2id login, constant-time failure, 10/min/IP), AUTH-2 (EdDSA JWT 15 min,
//! rotating refresh families with reuse detection, logout), and access-token rejection.
//! HTTP-level through the real router; needs `DATABASE_URL` (skipped otherwise) except the
//! tests marked "no database".

mod common;

use asd_backend::app::AppState;
use asd_contract::api_v1::Role;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct Ctx {
    db: TestDb,
    state: AppState,
    app: Router,
    org: Uuid,
    user: Uuid,
}

const EMAIL: &str = "Clinician@Example.org";

async fn setup() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let user = seed_user(&db.pool, Some(org), Role::Clinician, EMAIL).await;
    Some(Ctx {
        db,
        state,
        app,
        org,
        user,
    })
}

async fn login(app: &Router, email: &str, password: &str) -> (StatusCode, Value) {
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

async fn refresh(app: &Router, token: &str) -> (StatusCode, Value) {
    let (status, _, body) = send(
        app.clone(),
        json(
            Method::POST,
            "/v1/auth/refresh",
            &json!({"refresh_token": token}),
            None,
        ),
    )
    .await;
    (status, body)
}

fn jwt_part(token: &str, index: usize) -> Value {
    let part = token.split('.').nth(index).expect("three JWT parts");
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).expect("base64url")).expect("JSON")
}

fn sha256(s: &str) -> Vec<u8> {
    Sha256::digest(s.as_bytes()).to_vec()
}

#[tokio::test]
async fn login_returns_a_token_pair_with_a_15_minute_eddsa_access_token() {
    let Some(c) = setup().await else { return };
    let (status, body) = login(&c.app, EMAIL, PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    assert_eq!(body["expires_in"], 900);
    assert_eq!(body["user"]["id"], c.user.to_string());
    // As stored (seeded directly here; the users API stores addresses lower-cased).
    assert_eq!(body["user"]["email"], EMAIL);
    assert_eq!(body["user"]["role"], "clinician");
    assert_eq!(body["user"]["org_id"], c.org.to_string());

    let access = body["access_token"].as_str().unwrap();
    let header = jwt_part(access, 0);
    assert_eq!(header["alg"], "EdDSA");
    let claims = jwt_part(access, 1);
    assert_eq!(claims["sub"], c.user.to_string());
    assert_eq!(claims["org"], c.org.to_string());
    assert_eq!(claims["role"], "clinician");
    assert!(Uuid::parse_str(claims["jti"].as_str().unwrap()).is_ok());
    let (iat, exp) = (
        claims["iat"].as_i64().unwrap(),
        claims["exp"].as_i64().unwrap(),
    );
    assert_eq!(exp - iat, 900, "AUTH-2: 15 minutes");
    let now = chrono::Utc::now().timestamp();
    assert!((iat - now).abs() < 30);

    // Refresh token: opaque, 32 random bytes base64url, stored only as SHA-256.
    let rt = body["refresh_token"].as_str().unwrap();
    let raw = rt.strip_prefix("rt_").expect("rt_ prefix");
    assert_eq!(URL_SAFE_NO_PAD.decode(raw).unwrap().len(), 32);
    let (stored, ttl_days): (i64, f64) = sqlx::query_as(
        "SELECT count(*), max(extract(epoch FROM expires_at - now()) / 86400)::float8 \
         FROM refresh_tokens WHERE token_hash = $1 AND user_id = $2",
    )
    .bind(sha256(rt))
    .bind(c.user)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(stored, 1);
    assert!(
        (ttl_days - 30.0).abs() < 0.01,
        "30-day refresh, got {ttl_days}"
    );

    let last_login: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT last_login_at FROM users WHERE id = $1")
            .bind(c.user)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert!(last_login.is_some());

    // The access token authenticates a user-plane route.
    let (status, _, org) = send(
        c.app.clone(),
        empty(Method::GET, &format!("/v1/orgs/{}", c.org), Some(access)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{org}");
    assert_eq!(org["id"], c.org.to_string());
    c.db.cleanup().await;
}

#[tokio::test]
async fn stored_password_hash_is_argon2id_19mib_t2_p1() {
    let Some(c) = setup().await else { return };
    let root = seed_user(&c.db.pool, None, Role::Superadmin, "root@example.org").await;
    let (status, body) = send(
        c.app.clone(),
        json(
            Method::POST,
            &format!("/v1/orgs/{}/users", c.org),
            &json!({"email": "new@example.org", "role": "viewer", "password": "a long enough password"}),
            Some(&access_token(&c.state, root, None, Role::Superadmin)),
        ),
    )
    .await
    .into_status_body();
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let hash: String =
        sqlx::query_scalar("SELECT pw_hash FROM users WHERE email = 'new@example.org'")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert!(
        hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
        "{hash}"
    );
    assert!(!hash.contains("a long enough password"));
    let (status, _) = login(&c.app, "new@example.org", "a long enough password").await;
    assert_eq!(status, StatusCode::OK);
    c.db.cleanup().await;
}

trait IntoStatusBody {
    fn into_status_body(self) -> (StatusCode, Value);
}
impl IntoStatusBody for (StatusCode, http::HeaderMap, Value) {
    fn into_status_body(self) -> (StatusCode, Value) {
        (self.0, self.2)
    }
}

#[tokio::test]
async fn wrong_password_unknown_email_and_disabled_user_get_the_same_401() {
    let Some(c) = setup().await else { return };
    let (s1, b1) = login(&c.app, EMAIL, "wrong password").await;
    let (s2, b2) = login(&c.app, "nobody@example.org", PASSWORD).await;
    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = $1")
        .bind(c.user)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s3, b3) = login(&c.app, EMAIL, PASSWORD).await;
    for (s, b) in [(s1, &b1), (s2, &b2), (s3, &b3)] {
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{b}");
        assert_eq!(slug(b), "unauthorized");
    }
    // No hint which part was wrong.
    assert_eq!(b1, b2);
    assert_eq!(b2, b3);
    c.db.cleanup().await;
}

#[tokio::test]
async fn login_email_is_case_insensitive_and_malformed_bodies_are_problems() {
    let Some(c) = setup().await else { return };
    let (status, _) = login(&c.app, "  clinician@EXAMPLE.org ", PASSWORD).await;
    assert_eq!(status, StatusCode::OK);

    let (status, headers, body) = send(
        c.app.clone(),
        json(
            Method::POST,
            "/v1/auth/login",
            &json!({"email": EMAIL}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(content_type(&headers), "application/problem+json");
    assert_eq!(slug(&body), "validation-failed");
    c.db.cleanup().await;
}

#[tokio::test]
async fn login_is_rate_limited_to_10_per_minute_per_ip() {
    let Some(c) = setup().await else { return };
    let attempt = |ip: &'static str| {
        let app = c.app.clone();
        async move {
            send(
                app,
                from_ip(
                    json(
                        Method::POST,
                        "/v1/auth/login",
                        &json!({"email": "nobody@example.org", "password": "x"}),
                        None,
                    ),
                    ip,
                ),
            )
            .await
        }
    };
    for i in 0..10 {
        let (status, _, _) = attempt("203.0.113.7").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "attempt {i}");
    }
    let (status, headers, body) = attempt("203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(slug(&body), "rate-limited");
    let retry: u64 = headers[http::header::RETRY_AFTER]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&retry), "Retry-After {retry}");
    // Another client is not affected.
    let (status, _, _) = attempt("203.0.113.8").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    c.db.cleanup().await;
}

#[tokio::test]
async fn refresh_rotates_the_token_and_the_new_one_works() {
    let Some(c) = setup().await else { return };
    let (_, first) = login(&c.app, EMAIL, PASSWORD).await;
    let rt1 = first["refresh_token"].as_str().unwrap().to_owned();

    let (status, second) = refresh(&c.app, &rt1).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let rt2 = second["refresh_token"].as_str().unwrap().to_owned();
    assert_ne!(rt1, rt2);
    assert_eq!(second["expires_in"], 900);
    assert_eq!(second["user"]["id"], c.user.to_string());
    assert_ne!(second["access_token"], first["access_token"]);

    // Same family, old one marked rotated.
    let rows: Vec<(Uuid, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT family_id, rotated_at FROM refresh_tokens WHERE token_hash = ANY($1) \
         ORDER BY created_at",
    )
    .bind(vec![sha256(&rt1), sha256(&rt2)])
    .fetch_all(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, rows[1].0, "one family");

    let (status, third) = refresh(&c.app, &rt2).await;
    assert_eq!(status, StatusCode::OK, "{third}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn reusing_a_rotated_refresh_token_revokes_the_whole_family() {
    // Acceptance 10.
    let Some(c) = setup().await else { return };
    let (_, first) = login(&c.app, EMAIL, PASSWORD).await;
    let rt1 = first["refresh_token"].as_str().unwrap().to_owned();
    let (_, second) = refresh(&c.app, &rt1).await;
    let rt2 = second["refresh_token"].as_str().unwrap().to_owned();

    // A second session of the same user (another family) must survive.
    let (_, other) = login(&c.app, EMAIL, PASSWORD).await;
    let other_rt = other["refresh_token"].as_str().unwrap().to_owned();

    let (status, body) = refresh(&c.app, &rt1).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, _) = refresh(&c.app, &rt2).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "newest token of the family is revoked too"
    );

    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM refresh_tokens t JOIN refresh_tokens r ON r.family_id = t.family_id \
         WHERE t.token_hash = $1 AND r.revoked_at IS NULL",
    )
    .bind(sha256(&rt1))
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(live, 0);
    assert_eq!(
        audit_count(&c.db.pool, "auth.refresh_reuse_detected").await,
        1
    );

    let (status, _) = refresh(&c.app, &other_rt).await;
    assert_eq!(status, StatusCode::OK, "other families are untouched");
    c.db.cleanup().await;
}

#[tokio::test]
async fn unknown_expired_and_disabled_user_refresh_tokens_are_401() {
    let Some(c) = setup().await else { return };
    let (status, _) = refresh(&c.app, "rt_not-a-real-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, pair) = login(&c.app, EMAIL, PASSWORD).await;
    let rt = pair["refresh_token"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE refresh_tokens SET expires_at = now() - interval '1 second'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (status, _) = refresh(&c.app, &rt).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "expired");

    let (_, pair) = login(&c.app, EMAIL, PASSWORD).await;
    let rt = pair["refresh_token"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = $1")
        .bind(c.user)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (status, _) = refresh(&c.app, &rt).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "disabled user");
    c.db.cleanup().await;
}

#[tokio::test]
async fn logout_revokes_the_family_and_is_204_even_for_unknown_tokens() {
    let Some(c) = setup().await else { return };
    let (_, pair) = login(&c.app, EMAIL, PASSWORD).await;
    let rt = pair["refresh_token"].as_str().unwrap().to_owned();

    let (status, _, body) = send(
        c.app.clone(),
        json(
            Method::POST,
            "/v1/auth/logout",
            &json!({"refresh_token": rt}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);
    let (status, _) = refresh(&c.app, &rt).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(audit_count(&c.db.pool, "auth.logout").await, 1);

    let (status, _, _) = send(
        c.app.clone(),
        json(
            Method::POST,
            "/v1/auth/logout",
            &json!({"refresh_token": "rt_unknown"}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(audit_count(&c.db.pool, "auth.logout").await, 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn concurrent_refreshes_of_one_token_yield_at_most_one_success() {
    let Some(c) = setup().await else { return };
    let (_, pair) = login(&c.app, EMAIL, PASSWORD).await;
    let rt = pair["refresh_token"].as_str().unwrap().to_owned();
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let app = c.app.clone();
            let rt = rt.clone();
            tokio::spawn(async move { refresh(&app, &rt).await.0 })
        })
        .collect();
    let mut ok = 0;
    for t in tasks {
        let s = t.await.unwrap();
        assert!(s == StatusCode::OK || s == StatusCode::UNAUTHORIZED, "{s}");
        ok += usize::from(s == StatusCode::OK);
    }
    assert!(ok <= 1, "a refresh token is single-use, got {ok} successes");
    c.db.cleanup().await;
}

#[tokio::test]
async fn bad_access_tokens_are_401() {
    let Some(c) = setup().await else { return };
    let uri = format!("/v1/orgs/{}", c.org);
    let good = access_token(&c.state, c.user, Some(c.org), Role::Clinician);

    // Tampered payload (role escalated), signature kept.
    let mut parts: Vec<String> = good.split('.').map(str::to_owned).collect();
    let mut claims = jwt_part(&good, 1);
    claims["role"] = json!("owner");
    parts[1] = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
    let tampered = parts.join(".");

    // alg=none.
    let none = format!(
        "{}.{}.",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#),
        parts[1]
    );

    // Signed with another key.
    let other_state = {
        let mut cfg = (*c.state.config).clone();
        let key = asd_contract::signing::signing_key_from_seed([7u8; 32]);
        let pem = asd_contract::signing::signing_key_to_pkcs8_pem(&key).unwrap();
        cfg.auth.jwt = Some(asd_backend::auth::jwt::JwtKeys::from_key_str(&pem).unwrap());
        AppState::new(c.db.pool.clone(), c.state.blobs.clone(), cfg)
    };
    let foreign = access_token(&other_state, c.user, Some(c.org), Role::Clinician);

    // Expired.
    let expired = c
        .state
        .config
        .auth
        .jwt
        .as_ref()
        .unwrap()
        .issue_at(
            c.user,
            Some(c.org),
            Role::Clinician,
            chrono::Utc::now().timestamp() - 3600,
        )
        .unwrap()
        .0;

    for (name, token) in [
        ("tampered", tampered.as_str()),
        ("alg none", none.as_str()),
        ("foreign key", foreign.as_str()),
        ("expired", expired.as_str()),
        ("garbage", "not.a.jwt"),
        (
            "device token",
            "dvc_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ),
    ] {
        let (status, _, body) = send(c.app.clone(), empty(Method::GET, &uri, Some(token))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{name}: {body}");
        assert_eq!(slug(&body), "unauthorized", "{name}");
    }
    let (status, _, _) = send(c.app.clone(), empty(Method::GET, &uri, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no bearer");

    // The good one works, and stops working the moment the user is disabled.
    let (status, _, _) = send(c.app.clone(), empty(Method::GET, &uri, Some(&good))).await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = $1")
        .bind(c.user)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (status, _, _) = send(c.app.clone(), empty(Method::GET, &uri, Some(&good))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "disabled user");
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_role_comes_from_the_database_not_the_token() {
    // A demoted user's still-valid token must not keep the old role.
    let Some(c) = setup().await else { return };
    let owner_token = access_token(&c.state, c.user, Some(c.org), Role::Owner);
    let (status, _, body) = send(
        c.app.clone(),
        json(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            &json!({"name": "Renamed"}),
            Some(&owner_token),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "user is a clinician in the DB: {body}"
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn without_a_jwt_key_login_is_503_not_a_panic() {
    // No database needed: the key check comes first.
    let mut state = state_without_db();
    let mut cfg = (*state.config).clone();
    cfg.auth.jwt = None;
    state = AppState::new(state.db.clone(), state.blobs.clone(), cfg);
    let (status, _, body) = send(
        router(state),
        json(
            Method::POST,
            "/v1/auth/login",
            &json!({"email": EMAIL, "password": PASSWORD}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
}

#[tokio::test]
async fn user_routes_without_a_bearer_are_401_without_touching_the_database() {
    let id = Uuid::new_v4();
    for (method, uri) in [
        (Method::GET, format!("/v1/orgs/{id}")),
        (Method::PATCH, format!("/v1/orgs/{id}")),
        (Method::GET, format!("/v1/orgs/{id}/users")),
        (Method::GET, format!("/v1/orgs/{id}/devices")),
        (Method::POST, format!("/v1/orgs/{id}/devices/enrol-code")),
        (Method::POST, format!("/v1/devices/{id}/revoke")),
        (Method::GET, format!("/v1/orgs/{id}/audit")),
    ] {
        let (status, headers, _) = send(
            router(state_without_db()),
            empty(method.clone(), &uri, None),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(content_type(&headers), "application/problem+json");
    }
}

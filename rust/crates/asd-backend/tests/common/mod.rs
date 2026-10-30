//! owner: WP-B.1
//!
//! Shared helpers for `asd-backend` integration tests.
//!
//! Database tests use [`TestDb`]: one fresh Postgres database per test, created on the
//! server named by `DATABASE_URL`, migrated with the embedded migrator and dropped by
//! [`TestDb::cleanup`]. Without `DATABASE_URL` a database test is skipped (it prints
//! `SKIP`), unless `ASD_REQUIRE_ALL=1` is set (CI), where it fails instead.
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use std::sync::Arc;

use asd_backend::app::{self, AppState};
use asd_backend::blob::{BlobStore, MemoryBlobStore};
use asd_backend::config::Config;
use asd_backend::db;
use axum::body::Body;
use axum::Router;
use http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, ConnectOptions, Connection, PgConnection, PgPool};
use tower::ServiceExt;

/// A database URL that refuses connections quickly (port 1 on loopback).
pub const UNREACHABLE_DB_URL: &str = "postgres://asd:asd@127.0.0.1:1/asd_unreachable";

pub mod auth; // WP-B.2 helpers: seed users/devices, mint tokens, JSON requests

/// Config for tests: memory blob store, a JWT signing key generated for this test process
/// (never a committed secret), everything else at defaults.
pub fn test_config(database_url: &str) -> Config {
    Config::from_lookup(|key| match key {
        "DATABASE_URL" => Some(database_url.to_owned()),
        "ASD_BLOB_BACKEND" => Some("memory".to_owned()),
        "ASD_DB_ACQUIRE_TIMEOUT_MS" => Some("500".to_owned()),
        "ASD_JWT_SIGNING_KEY" => Some(test_jwt_key().to_owned()),
        _ => None,
    })
    .expect("test config is valid")
}

/// A fresh Ed25519 PKCS#8 PEM per test process (WP-B.2, AUTH-2).
pub fn test_jwt_key() -> &'static str {
    static KEY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    KEY.get_or_init(|| {
        let mut seed = [0u8; 32];
        seed[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        seed[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        let key = asd_contract::signing::signing_key_from_seed(seed);
        asd_contract::signing::signing_key_to_pkcs8_pem(&key).expect("pem")
    })
}

/// App state whose pool points at an unreachable server (no Postgres needed).
pub fn state_without_db() -> AppState {
    state_with_blobs_without_db(Arc::new(MemoryBlobStore::new()))
}

pub fn state_with_blobs_without_db(blobs: Arc<dyn BlobStore>) -> AppState {
    let config = test_config(UNREACHABLE_DB_URL);
    let pool = db::connect_lazy(&config).expect("lazy pool");
    AppState::new(pool, blobs, config)
}

pub fn state_with_pool(pool: PgPool) -> AppState {
    let config = test_config(UNREACHABLE_DB_URL);
    AppState::new(pool, Arc::new(MemoryBlobStore::new()), config)
}

pub fn router(state: AppState) -> Router {
    app::router(state)
}

/// Sends one request through the router; returns status, headers and the body as JSON
/// (`Value::Null` for an empty body).
pub async fn send(
    router: Router,
    request: Request<Body>,
) -> (StatusCode, HeaderMap, serde_json::Value) {
    let response = router.oneshot(request).await.expect("router is infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            panic!(
                "body is not JSON ({e}): {}",
                String::from_utf8_lossy(&bytes)
            )
        })
    };
    (status, headers, body)
}

pub fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request builds")
}

pub fn content_type(headers: &HeaderMap) -> &str {
    headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

/// One throwaway database per test.
pub struct TestDb {
    pub pool: PgPool,
    pub name: String,
    server: PgConnectOptions,
}

impl TestDb {
    /// Fresh database with every migration applied; `None` (skip) without `DATABASE_URL`.
    pub async fn migrated() -> Option<Self> {
        let db = Self::empty().await?;
        db::MIGRATOR.run(&db.pool).await.expect("migrations apply");
        Some(db)
    }

    /// Fresh database with no migrations applied.
    pub async fn empty() -> Option<Self> {
        let url = database_url_or_skip()?;
        let server: PgConnectOptions = url.parse().expect("DATABASE_URL parses");
        let name = format!("asd_t_{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::connect_with(&server)
            .await
            .expect("connect to DATABASE_URL");
        // The name is generated here (hex), never user input.
        sqlx::query(AssertSqlSafe(format!(r#"CREATE DATABASE "{name}""#)))
            .execute(&mut admin)
            .await
            .expect("create test database");
        admin.close().await.ok();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(server.clone().database(&name).disable_statement_logging())
            .await
            .expect("connect to test database");
        Some(Self { pool, name, server })
    }

    /// Closes the pool and drops the database. Call at the end of every test.
    pub async fn cleanup(self) {
        self.pool.close().await;
        let mut admin = PgConnection::connect_with(&self.server)
            .await
            .expect("connect to DATABASE_URL");
        sqlx::query(AssertSqlSafe(format!(
            r#"DROP DATABASE IF EXISTS "{}""#,
            self.name
        )))
        .execute(&mut admin)
        .await
        .expect("drop test database");
        admin.close().await.ok();
    }
}

fn database_url_or_skip() -> Option<String> {
    match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            if std::env::var("ASD_REQUIRE_ALL").as_deref() == Ok("1") {
                panic!("DATABASE_URL is required (ASD_REQUIRE_ALL=1)");
            }
            eprintln!("SKIP: DATABASE_URL not set; database test skipped");
            None
        }
    }
}

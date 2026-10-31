//! owner: WP-C.11
//!
//! Shared fixture for the participant detail, timeline, MRN and note tests: an org, users
//! with tokens, participants and sessions inserted with SQL (the routes under test never
//! ingest), and a JSON request helper.
#![allow(dead_code)]

use asd_backend::app::AppState;
use asd_contract::api_v1::Role;
use axum::Router;
use chrono::{DateTime, Utc};
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::auth::*;
use crate::common::*;

pub struct Ctx {
    pub db: TestDb,
    pub state: AppState,
    pub app: Router,
    pub org: Uuid,
}

impl Ctx {
    pub async fn new() -> Option<Self> {
        let db = TestDb::migrated().await?;
        let state = state_with_pool(db.pool.clone());
        let app = router(state.clone());
        let org = seed_org(&db.pool, "Clinic A").await;
        Some(Self {
            db,
            state,
            app,
            org,
        })
    }

    /// A user of `org` (none for superadmin) and its access token.
    pub async fn token_in(&self, org: Option<Uuid>, role: Role) -> (Uuid, String) {
        let email = format!("{}-{}@example.org", role_str(role), Uuid::new_v4().simple());
        let id = seed_user(&self.db.pool, org, role, &email).await;
        (id, access_token(&self.state, id, org, role))
    }

    pub async fn token(&self, role: Role) -> (Uuid, String) {
        let org = (role != Role::Superadmin).then_some(self.org);
        self.token_in(org, role).await
    }

    pub async fn call(
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

    /// Sets `identity.patient_ref_mode` directly (no audit row).
    pub async fn set_mode(&self, org: Uuid, mode: &str) {
        sqlx::query("UPDATE organizations SET settings = settings || $2::jsonb WHERE id = $1")
            .bind(org)
            .bind(json!({"identity": {"patient_ref_mode": mode}}))
            .execute(&self.db.pool)
            .await
            .unwrap();
    }

    pub async fn participant(&self, org: Uuid, token: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO participants (org_id, token, age_bracket_at_enrol, language)
             VALUES ($1, $2, '24-36m', 'en') RETURNING id",
        )
        .bind(org)
        .bind(token)
        .fetch_one(&self.db.pool)
        .await
        .unwrap()
    }

    pub async fn session(&self, org: Uuid, participant: Uuid, id: &str, started: &str) {
        let at: DateTime<Utc> = started.parse().unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, org_id, participant_id, participant_token, capture_source,
                started_at, duration_sec, schema_version, quality_verdict, valid_frame_ratio,
                social_preference, attention_flag, summary)
             SELECT $1, $2, p.id, p.token, 'truedepth', $3, 30, '1.6', 'good', 0.9, 0.5, 'red', '{}'::jsonb
             FROM participants p WHERE p.id = $4",
        )
        .bind(id)
        .bind(org)
        .bind(at)
        .bind(participant)
        .execute(&self.db.pool)
        .await
        .unwrap();
    }

    pub async fn audit_rows(&self, action: &str) -> Vec<Value> {
        sqlx::query_scalar::<_, Value>(
            "SELECT to_jsonb(a) FROM audit_log a WHERE action = $1 ORDER BY id",
        )
        .bind(action)
        .fetch_all(&self.db.pool)
        .await
        .unwrap()
    }

    /// Every audit row, as one string (to assert a value never appears).
    pub async fn audit_dump(&self) -> String {
        let rows: Vec<Value> =
            sqlx::query_scalar("SELECT to_jsonb(a) FROM audit_log a ORDER BY id")
                .fetch_all(&self.db.pool)
                .await
                .unwrap();
        serde_json::to_string(&rows).unwrap()
    }
}

pub fn slug_of(body: &Value) -> &str {
    body["type"]
        .as_str()
        .and_then(|t| t.rsplit('/').next())
        .unwrap_or("")
}

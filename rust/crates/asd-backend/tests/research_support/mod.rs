//! owner: WP-X.2
//!
//! Shared fixture of the researcher-toolkit tests (`research_frames`, `experiments`,
//! `cohorts`, `config_docs`, `config_effective`): an org (and a second one), users with
//! tokens, sessions inserted with SQL where the route under test never ingests, a research
//! frame log built from the asd-core raw-log fixture, finished analysis runs inserted with SQL
//! (the job is WP-X.3), and the worker.
#![allow(dead_code)]

use std::time::Duration;

use asd_backend::app::AppState;
use asd_backend::jobs::{runner, JobContext};
use asd_contract::api_v1::Role;
use axum::body::Body;
use axum::Router;
use base64::Engine as _;
use http::{Method, Request, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::common::auth::*;
use crate::common::*;

/// The asd-core raw tracking fixture (`asd-raw-tracking/1`, header + 3 samples).
pub const RAW_LOG: &str =
    include_str!("../../../asd-core/tests/fixtures/raw_tracking_sample.jsonl");

pub struct Ctx {
    pub db: TestDb,
    pub state: AppState,
    pub app: Router,
    pub org: Uuid,
    pub other: Uuid,
}

impl Ctx {
    pub async fn new() -> Option<Self> {
        let db = TestDb::migrated().await?;
        let state = state_with_pool(db.pool.clone());
        Some(Self::with_state(db, state).await)
    }

    pub async fn with_state(db: TestDb, state: AppState) -> Self {
        let app = router(state.clone());
        let org = seed_org(&db.pool, "Clinic A").await;
        let other = seed_org(&db.pool, "Clinic B").await;
        Self {
            db,
            state,
            app,
            org,
            other,
        }
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

    /// `PATCH` with `If-Match: "<version>"`.
    pub async fn patch_if(
        &self,
        uri: &str,
        body: Value,
        if_match: Option<&str>,
        bearer: &str,
    ) -> (StatusCode, Value) {
        let mut req = json(Method::PATCH, uri, &body, Some(bearer));
        if let Some(v) = if_match {
            req.headers_mut()
                .insert(http::header::IF_MATCH, v.parse().unwrap());
        }
        let (s, _, b) = send(self.app.clone(), req).await;
        (s, b)
    }

    pub async fn participant(&self, org: Uuid, token: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO participants (org_id, token) VALUES ($1, $2)
             ON CONFLICT (org_id, token) DO UPDATE SET token = EXCLUDED.token RETURNING id",
        )
        .bind(org)
        .bind(token)
        .fetch_one(&self.db.pool)
        .await
        .unwrap()
    }

    /// A session of `org` inserted with SQL (see [`Seed`]).
    pub async fn seed(&self, org: Uuid, s: &Seed) {
        let p = self.participant(org, &s.token).await;
        let mut summary = json!({"sessionId": s.id});
        if let Some(c) = s.consented {
            summary["normCollection"] = json!({"consented": c});
        }
        if let Some(a) = s.age_months {
            summary["ageMonths"] = json!(a);
        }
        sqlx::query(
            "INSERT INTO sessions (id, org_id, participant_id, participant_token, capture_source,
                started_at, received_at, duration_sec, schema_version, quality_verdict,
                valid_frame_ratio, attention_flag, summary, age_months, tracking_verdict,
                scoring_status, device_id, social_preference)
             VALUES ($1, $2, $3, $4, $5, now() - make_interval(days => $6),
                     now() - make_interval(days => $6), 30, '1.8', $7, $8, 'green', $9, $10,
                     $11, $12, $13,
                     CASE WHEN $12 IS DISTINCT FROM 'unscorable' THEN 0.5 END)",
        )
        .bind(&s.id)
        .bind(org)
        .bind(p)
        .bind(&s.token)
        .bind(&s.capture_source)
        .bind(s.days_ago)
        .bind(&s.quality)
        .bind(s.valid)
        .bind(summary)
        .bind(s.age_months)
        .bind(&s.tracking)
        .bind(&s.scoring_status)
        .bind(s.device)
        .execute(&self.db.pool)
        .await
        .unwrap();
        if s.frames {
            sqlx::query(
                "UPDATE sessions SET research_frames_blob_key = 'k/' || id,
                        research_frames_bytes = 10, research_frames_sha256 = $2
                 WHERE id = $1",
            )
            .bind(&s.id)
            .bind(vec![0u8; 32])
            .execute(&self.db.pool)
            .await
            .unwrap();
        }
    }

    pub async fn withdraw(&self, org: Uuid, token: &str) {
        sqlx::query(
            "UPDATE participants SET withdrawn_at = now() WHERE org_id = $1 AND token = $2",
        )
        .bind(org)
        .bind(token)
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

    /// The worker's context: same pool, blob store and keyring as the router.
    pub fn job_ctx(&self) -> JobContext {
        JobContext::from_state(&self.state).with_heartbeat_every(Duration::from_millis(50))
    }

    pub async fn run_jobs(&self) -> usize {
        runner::run_pending_in(&self.job_ctx(), "test-worker")
            .await
            .unwrap()
    }

    /// A `succeeded` analysis run (inserted with SQL: the job is WP-X.3) over `profile_hash`
    /// or `baselines_version`; links it as the experiment's last run when one is named.
    pub async fn finished_run(
        &self,
        org: Uuid,
        experiment: Option<Uuid>,
        profile_hash: Vec<u8>,
        baselines_version: &str,
    ) -> Uuid {
        let run: Uuid = sqlx::query_scalar(
            "INSERT INTO analysis_runs
               (org_id, kind, status, experiment_id, definition, definition_hash, cohort_hash,
                profile_hash, published_profile_version, baselines_version, catalog_version,
                core_version, contract_version, pipeline_version, registry_version, seed,
                level_requested, finished_at)
             VALUES ($1, 'cohort_rescore', 'succeeded', $2, '{}', $3, $3, $4, 'p', $5, 'c',
                     '0', '0', '0', '1.0.0', 7, 'auto', now())
             RETURNING id",
        )
        .bind(org)
        .bind(experiment)
        .bind(vec![1u8; 32])
        .bind(profile_hash)
        .bind(baselines_version)
        .fetch_one(&self.db.pool)
        .await
        .unwrap();
        if let Some(e) = experiment {
            sqlx::query("UPDATE experiments SET last_run_id = $2 WHERE id = $1")
                .bind(e)
                .bind(run)
                .execute(&self.db.pool)
                .await
                .unwrap();
        }
        run
    }

    /// `organizations.settings.research.small_cell_k` through the org route.
    pub async fn set_small_cell_k(&self, k: u32, owner: &str) -> (StatusCode, Value) {
        self.call(
            Method::PATCH,
            &format!("/v1/orgs/{}", self.org),
            Some(json!({"settings": {"research": {"small_cell_k": k}}})),
            owner,
        )
        .await
    }
}

/// A session to insert: an adult default that every cohort admits (consented, 30 months,
/// scored, optimal and good tracking, 0.9 valid, real capture, received 10 days ago).
#[derive(Debug, Clone)]
pub struct Seed {
    pub id: String,
    pub token: String,
    pub age_months: Option<i32>,
    pub consented: Option<bool>,
    pub days_ago: i32,
    pub frames: bool,
    pub quality: String,
    pub tracking: Option<String>,
    pub valid: f64,
    pub scoring_status: Option<String>,
    pub capture_source: String,
    pub device: Option<Uuid>,
}

impl Seed {
    pub fn new(id: &str, age_months: i32) -> Self {
        Self {
            id: id.into(),
            token: format!("TOK-{id}"),
            age_months: Some(age_months),
            consented: Some(true),
            days_ago: 10,
            frames: false,
            quality: "optimal".into(),
            tracking: Some("good".into()),
            valid: 0.9,
            scoring_status: Some("scored".into()),
            capture_source: "truedepth".into(),
            device: None,
        }
    }
}

/// The raw-log fixture with `sessionId` in its header (RFL-1, RFL-3).
pub fn research_log(session_id: &str) -> Vec<u8> {
    let mut lines = RAW_LOG.lines();
    let mut header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    header["sessionId"] = json!(session_id);
    let mut out = serde_json::to_string(&header).unwrap();
    out.push('\n');
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
    out.into_bytes()
}

/// `sha-256=:<base64>:` of `body` (RFC 9530).
pub fn digest(body: &[u8]) -> String {
    format!(
        "sha-256=:{}:",
        base64::engine::general_purpose::STANDARD.encode(Sha256::digest(body))
    )
}

/// `PUT /v1/sessions/{id}/blobs/research_frames` as the device.
pub fn put_research(
    device: &str,
    session: &str,
    body: Vec<u8>,
    content_digest: Option<&str>,
) -> Request<Body> {
    let mut b = Request::builder()
        .method(Method::PUT)
        .uri(format!("/v1/sessions/{session}/blobs/research_frames"))
        .header(http::header::AUTHORIZATION, format!("Bearer {device}"))
        .header(http::header::CONTENT_TYPE, "application/x-ndjson");
    if let Some(d) = content_digest {
        b = b.header("content-digest", d);
    }
    b.body(Body::from(body)).unwrap()
}

/// sha256 of the RFC 8785 form (the hash `experiments.profile_hash` and GOV-3 use).
pub fn jcs_hash(v: &Value) -> Vec<u8> {
    asd_backend::config_docs::lifecycle::body_hash(v)
}

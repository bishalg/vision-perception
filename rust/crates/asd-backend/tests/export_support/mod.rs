//! owner: WP-I.6
//!
//! Fixture for the export tests on top of `participant_support`: stored session records
//! (`sessions.summary`) that decode as `GazeSessionRecord`, research consent, a job
//! context sharing the router's blob store, and a "request then run" helper.
#![allow(dead_code)]

use std::time::Duration;

use asd_backend::jobs::{runner, JobContext};
use http::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::participant_support::Ctx;

/// A stored live (`truedepth`) session record; `consented` sets
/// `normCollection.consented` (absent when `None`).
pub fn gaze_summary(session_id: &str, token: &str, consented: Option<bool>) -> Value {
    let mut v = json!({
        "id": format!("REC-{session_id}"),
        "sessionId": session_id,
        "participantId": token,
        "captureSource": "truedepth",
        "startedAt": "2026-10-01T10:00:00.000Z",
        "endedAt": "2026-10-01T10:00:30.000Z",
        "durationSec": 30,
        "socialPreferenceScore": 0.42,
        "validFrameRatio": 0.9,
        "eyeDwellRatio": 0.55,
        "firstEyeLatencyMs": 310,
        "calibratedDistanceCm": 35,
        "qualityVerdict": "optimal",
        "socialSmileIntensity": 0.3,
        "smileReactivityIndex": 0.25,
        "flatAffectScore": 0.2,
        "surpriseScoreMean": 0.1,
        "schemaVersion": "v1.6",
        "zeroRawVideoEnforced": true,
        "perVideoComposite": 0.6,
        "perVideo": []
    });
    if let Some(c) = consented {
        v["normCollection"] = json!({"consented": c});
    }
    v
}

impl Ctx {
    /// A participant with one session whose stored record decodes; returns its id.
    pub async fn exportable_session(
        &self,
        participant: Uuid,
        token: &str,
        session: &str,
        started: &str,
        consented: Option<bool>,
    ) {
        self.session(self.org, participant, session, started).await;
        sqlx::query("UPDATE sessions SET summary = $2 WHERE id = $1")
            .bind(session)
            .bind(gaze_summary(session, token, consented))
            .execute(&self.db.pool)
            .await
            .unwrap();
    }

    pub async fn withdraw(&self, participant: Uuid) {
        sqlx::query("UPDATE participants SET withdrawn_at = now() WHERE id = $1")
            .bind(participant)
            .execute(&self.db.pool)
            .await
            .unwrap();
    }

    /// The worker's context: same pool and blob store as the router.
    pub fn job_ctx(&self) -> JobContext {
        JobContext::from_state(&self.state).with_heartbeat_every(Duration::from_millis(50))
    }

    pub async fn run_jobs(&self) -> usize {
        runner::run_pending_in(&self.job_ctx(), "test-worker")
            .await
            .unwrap()
    }

    /// `POST /v1/orgs/{org}/exports`; returns status and body.
    pub async fn request_export(&self, bearer: &str, body: Value) -> (StatusCode, Value) {
        self.call(
            Method::POST,
            &format!("/v1/orgs/{}/exports", self.org),
            Some(body),
            bearer,
        )
        .await
    }

    /// Requests an export, expects 202, runs the queue, returns the finished export.
    pub async fn export_and_run(&self, bearer: &str, body: Value) -> Value {
        let (s, made) = self.request_export(bearer, body).await;
        assert_eq!(s, StatusCode::ACCEPTED, "{made}");
        self.run_jobs().await;
        let (s, done) = self
            .call(
                Method::GET,
                &format!("/v1/exports/{}", made["id"].as_str().unwrap()),
                None,
                bearer,
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{done}");
        done
    }

    /// The stored artifact bytes of an export.
    pub async fn artifact(&self, export_id: &str) -> Vec<u8> {
        let key: String = sqlx::query_scalar("SELECT blob_key FROM exports WHERE id = $1")
            .bind(export_id.parse::<Uuid>().unwrap())
            .fetch_one(&self.db.pool)
            .await
            .unwrap();
        self.state
            .blobs
            .get(&key)
            .await
            .unwrap()
            .expect("artifact stored")
            .bytes
            .to_vec()
    }

    pub async fn export_items(&self, export_id: &str) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT session_id FROM export_items WHERE export_id = $1 ORDER BY session_id",
        )
        .bind(export_id.parse::<Uuid>().unwrap())
        .fetch_all(&self.db.pool)
        .await
        .unwrap()
    }
}

/// A valid request body.
pub fn body(format: &str, mode: &str, purpose: &str) -> Value {
    json!({
        "format": format,
        "mode": mode,
        "purpose": purpose,
        "reason": "Quarterly extract for the approved study",
    })
}

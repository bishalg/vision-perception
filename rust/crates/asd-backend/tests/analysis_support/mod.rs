//! owner: WP-X.3
//!
//! Fixture of the analysis-run tests (`analysis_runs.rs`, `jobs_cancel.rs`): an org with a
//! synthetic cohort of 30 research-consented sessions across the default age bands, each
//! carrying an `asd-cli synth` session record (`rawPayload.perVideo`), 10 of them with a real
//! sealed research frame log built from `asd-cli gaze synth` (`synth_tracking`, scenario
//! `ideal`) with header clips and AOIs, and an experiment whose profile changes one
//! score-level knob and one frames-level knob.
#![allow(dead_code)]

use std::sync::Arc;

use asd_backend::app::AppState;
use asd_backend::blob::MemoryBlobStore;
use asd_backend::config::Config;
use asd_backend::routes::blobs::{seal_research_log, SEALED_CONTENT_TYPE};
use asd_contract::api_v1::Role;
use asd_core::gaze::aoi_space::{NormBox, ScreenAoi};
use asd_core::gaze::raw_log;
use asd_core::synth_tracking::{generate, Scenario};
use asd_core::types::tracking::RawLogClip;
use bytes::Bytes;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::common::{self, TestDb};
use crate::research_support::{Ctx, Seed};

/// Sessions of the fixture cohort and how many carry a research log.
pub const COHORT_N: usize = 30;
pub const FRAMES_N: usize = 10;

/// A context whose configuration `tweak` adjusts (analysis limits, synthetic data).
pub async fn ctx_with(tweak: impl FnOnce(&mut Config)) -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let mut config = common::test_config(common::UNREACHABLE_DB_URL);
    tweak(&mut config);
    let state = AppState::new(db.pool.clone(), Arc::new(MemoryBlobStore::new()), config);
    Some(Ctx::with_state(db, state).await)
}

fn aoi(name: &str, x_min: f64, x_max: f64) -> ScreenAoi {
    ScreenAoi {
        name: name.to_owned(),
        start_sec: 0.0,
        end_sec: 60.0,
        rect: NormBox {
            x_min,
            y_min: 0.0,
            x_max,
            y_max: 1.0,
        },
        roles: Vec::new(),
    }
}

/// A synthetic research frame log for `session` (RFL-1: header clips with AOIs).
pub fn synth_log(session: &str, seed: u64) -> Vec<u8> {
    let mut log = generate(Scenario::Ideal, seed);
    let mut ids: Vec<String> = Vec::new();
    for s in &log.samples {
        if let Some(id) = &s.stimulus_id {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    log.header.clips = ids
        .iter()
        .enumerate()
        .map(|(order, id)| RawLogClip {
            clip_id: id.clone(),
            order: order as u32,
            catalog_version: None,
            aois: vec![
                aoi("Social_Actor", 0.0, 0.5),
                aoi("Geometric_Pattern", 0.5, 1.0),
            ],
            video_aspect: None,
        })
        .collect();
    log.header.session_id = Some(session.to_owned());
    raw_log::to_jsonl(&log).unwrap().into_bytes()
}

/// Seals `log` as the upload route does and records it on the session (RFL-3).
pub async fn attach_log(c: &Ctx, org: Uuid, session: &str, log: &[u8]) {
    let keyring = c.state.keyring.as_deref().expect("test config has a KEK");
    let sealed = seal_research_log(keyring, org, session, log).await.unwrap();
    let key = format!("research/{org}/{session}.jsonl.sealed");
    c.state
        .blobs
        .put_object_large(&key, SEALED_CONTENT_TYPE, Bytes::from(sealed))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE sessions SET research_frames_blob_key = $2, research_frames_bytes = $3,
                research_frames_sha256 = $4
         WHERE id = $1",
    )
    .bind(session)
    .bind(key)
    .bind(log.len() as i64)
    .bind(Sha256::digest(log).to_vec())
    .execute(&c.db.pool)
    .await
    .unwrap();
}

/// One research-consented session of `org` with a synth record (`perVideo`); `age` months.
pub async fn add_session(c: &Ctx, org: Uuid, id: &str, index: u64, age: i32, device: Option<Uuid>) {
    let mut seed = Seed::new(id, age);
    seed.device = device;
    c.seed(org, &seed).await;
    let synth = asd_core::synth::generate_session(7, index, None);
    let mut raw = synth["rawPayload"].clone();
    raw["sessionId"] = json!(id);
    raw["ageMonths"] = json!(age);
    raw["normCollection"] = json!({"consented": true});
    raw.as_object_mut().unwrap().remove("trackingQuality");
    sqlx::query("UPDATE sessions SET summary = $2, valid_frame_ratio = $3 WHERE id = $1")
        .bind(id)
        .bind(&raw)
        .bind(raw["validFrameRatio"].as_f64().unwrap_or(0.9))
        .execute(&c.db.pool)
        .await
        .unwrap();
}

/// The cohort fixture: `C-00`..`C-29`, ages 12..=47 across the bands, `C-00`..`C-09` with a
/// research log. `device` is set on the sessions listed in `on_device` (by index).
pub async fn cohort(c: &Ctx, device: Option<(Uuid, &[usize])>) {
    for i in 0..COHORT_N {
        let id = format!("C-{i:02}");
        let age = 12 + ((i * 7) % 36) as i32;
        let dev = device.and_then(|(d, on)| on.contains(&i).then_some(d));
        add_session(c, c.org, &id, i as u64, age, dev).await;
        if i < FRAMES_N {
            attach_log(c, c.org, &id, &synth_log(&id, 100 + i as u64)).await;
        }
    }
}

/// An experiment of the org (created and edited through the routes): I-VT velocity 2.0
/// (`frames` knob) and `lowConfidenceValidRatio` 0.7 (`score` knob). Returns `(id, body)`.
pub async fn experiment(c: &Ctx, bearer: &str) -> (String, Value) {
    let (s, exp) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.org),
            Some(json!({"name": format!("exp-{}", Uuid::new_v4().simple())})),
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{exp}");
    let id = exp["id"].as_str().unwrap().to_owned();
    let mut profile = exp["profile"].clone();
    profile["metrics"]["ivtVelocityUnitsPerSec"] = json!(2.0);
    profile["lowConfidenceValidRatio"] = json!(0.7);
    let (s, exp) = c
        .patch_if(
            &format!("/v1/experiments/{id}"),
            json!({"profile": profile}),
            Some("\"1\""),
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{exp}");
    (id, exp)
}

/// `POST /v1/orgs/{org}/analysis-runs`.
pub async fn create_run(c: &Ctx, body: Value, bearer: &str) -> (StatusCode, Value) {
    c.call(
        Method::POST,
        &format!("/v1/orgs/{}/analysis-runs", c.org),
        Some(body),
        bearer,
    )
    .await
}

pub async fn get_run(c: &Ctx, id: &str, bearer: &str) -> Value {
    let (s, b) = c
        .call(
            Method::GET,
            &format!("/v1/analysis-runs/{id}"),
            None,
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    b
}

/// `(id, xmin)` of every session, then of every review item, clip score and config document
/// (acceptance 10: nothing the toolkit reads is written).
pub async fn xmins(c: &Ctx) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT 's:' || id, xmin::text FROM sessions
         UNION ALL SELECT 'r:' || id::text, xmin::text FROM review_items
         UNION ALL SELECT 'c:' || id::text, xmin::text FROM config_documents
         ORDER BY 1",
    )
    .fetch_all(&c.db.pool)
    .await
    .unwrap()
}

/// Roles used by the RBAC rows.
pub const ORG_ROLES: [Role; 6] = [
    Role::Owner,
    Role::Admin,
    Role::Clinician,
    Role::Researcher,
    Role::Reviewer,
    Role::Viewer,
];

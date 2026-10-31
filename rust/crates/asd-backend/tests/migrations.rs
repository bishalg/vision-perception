//! owner: WP-B.1
//!
//! `migrations/0001_initial.sql` creates every table of `02 §3` plus the WP-B.1
//! additions (`enrol_codes`, `refresh_tokens`, `bundle_adoption`,
//! `sessions.participant_token`), and its constraints enforce the contract decisions that
//! belong in the schema (CON-13 capture source, CON-11 enrol-code uses, CON-15 flag
//! values, ADR-B5 idempotency insert-first). Needs `DATABASE_URL`; skipped otherwise.

mod common;

use std::collections::BTreeSet;

use asd_backend::db;
use common::TestDb;
use sqlx::PgPool;

const TABLES: &[&str] = &[
    "audit_log",
    "bundle_adoption",
    "care_events",
    // WP-C.9 (0008_clinical.sql); columns and triggers in migrations_0008.rs.
    "clinical_notes",
    "clinical_review_versions",
    "clinical_reviews",
    "clinical_signatures",
    "config_bundles",
    "config_documents",
    "devices",
    "enrol_codes",
    "erasure_requests",
    "idempotency_keys",
    "jobs",
    "labels",
    "norm_runs",
    "org_data_keys",
    "organizations",
    "participants",
    "patient_refs",
    "referral_letters",
    "refresh_tokens",
    "review_items",
    "session_clip_scores",
    "sessions",
    "stimulus_assets",
    "users",
];

async fn columns(pool: &PgPool, table: &str) -> BTreeSet<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .collect()
}

fn assert_has(cols: &BTreeSet<String>, table: &str, expected: &[&str]) {
    for c in expected {
        assert!(cols.contains(*c), "{table}.{c} missing; have {cols:?}");
    }
}

async fn org(pool: &PgPool) -> uuid::Uuid {
    sqlx::query_scalar(
        "INSERT INTO organizations (name, kind) VALUES ('Clinic A', 'clinic') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn participant(pool: &PgPool, org_id: uuid::Uuid, token: &str) -> uuid::Uuid {
    sqlx::query_scalar("INSERT INTO participants (org_id, token) VALUES ($1, $2) RETURNING id")
        .bind(org_id)
        .bind(token)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Inserts a minimal session; returns the database error message on failure.
async fn insert_session(
    pool: &PgPool,
    id: &str,
    org_id: uuid::Uuid,
    participant_id: uuid::Uuid,
    capture_source: &str,
    attention_flag: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO sessions (id, org_id, participant_id, participant_token, capture_source, \
           started_at, duration_sec, schema_version, quality_verdict, valid_frame_ratio, \
           social_preference, attention_flag, summary) \
         VALUES ($1, $2, $3, 'child-SESS-GOL', $4, now(), 32.5, '1.6', 'optimal', 0.93, 0.62, $5, '{}')",
    )
    .bind(id)
    .bind(org_id)
    .bind(participant_id)
    .bind(capture_source)
    .bind(attention_flag)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tokio::test]
async fn initial_migration_creates_every_table() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let tables: BTreeSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' \
           AND table_name NOT LIKE '\\_sqlx%'",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap()
    .into_iter()
    .collect();
    let expected: BTreeSet<String> = TABLES.iter().map(|t| t.to_string()).collect();
    assert_eq!(tables, expected);
    db.cleanup().await;
}

#[tokio::test]
async fn wp_b1_additions_have_their_columns() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let p = &db.pool;
    assert_has(
        &columns(p, "enrol_codes").await,
        "enrol_codes",
        &[
            "id",
            "org_id",
            "code_hash",
            "max_uses",
            "uses",
            "expires_at",
            "created_by",
            "revoked_at",
        ],
    );
    assert_has(
        &columns(p, "refresh_tokens").await,
        "refresh_tokens",
        &[
            "id",
            "user_id",
            "family_id",
            "token_hash",
            "expires_at",
            "rotated_at",
            "revoked_at",
        ],
    );
    assert_has(
        &columns(p, "bundle_adoption").await,
        "bundle_adoption",
        &["device_id", "bundle_version", "at"],
    );
    assert_has(
        &columns(p, "sessions").await,
        "sessions",
        &[
            "id",
            "org_id",
            "participant_id",
            "participant_token",
            "device_id",
            "capture_source",
            "started_at",
            "ended_at",
            "duration_sec",
            "battery",
            "age_pack",
            "lang",
            "bundle_version",
            "schema_version",
            "quality_verdict",
            "valid_frame_ratio",
            "calibrated_distance_cm",
            "social_preference",
            "eye_dwell",
            "mouth_dwell",
            "first_eye_latency_ms",
            "affect",
            "composite_score",
            "composite_confidence",
            "attention_flag",
            "summary",
            "frames_blob_key",
            "report_pdf_key",
            "received_at",
            "idempotency_key",
        ],
    );
    assert_has(
        &columns(p, "session_clip_scores").await,
        "session_clip_scores",
        &[
            "session_id",
            "clip_id",
            "clip_order",
            "dwell_social",
            "dwell_geometric",
            "dwell_eye",
            "dwell_mouth",
            "orienting_latency_ms",
            "gaze_cue_accuracy",
            "fixation_count",
            "mean_fixation_ms",
            "status",
            "score",
            "weight",
            "baseline_ref",
            "details",
        ],
    );
    assert_has(
        &columns(p, "idempotency_keys").await,
        "idempotency_keys",
        &[
            "key",
            "scope",
            "request_hash",
            "status_code",
            "response_body",
            "created_at",
        ],
    );
    assert_has(
        &columns(p, "audit_log").await,
        "audit_log",
        &[
            "id",
            "ts",
            "actor_type",
            "actor_id",
            "org_id",
            "action",
            "target_type",
            "target_id",
            "before",
            "after",
            "ip",
            "user_agent",
        ],
    );
    db.cleanup().await;
}

#[tokio::test]
async fn migrator_is_idempotent_and_records_every_version() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    db::MIGRATOR
        .run(&db.pool)
        .await
        .expect("second run is a no-op");
    assert!(db::pending_migrations(&db.pool).await.unwrap().is_empty());
    db.cleanup().await;
}

#[tokio::test]
async fn pending_migrations_lists_everything_on_an_empty_database() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pending = db::pending_migrations(&db.pool).await.unwrap();
    let all: Vec<i64> = db::MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(pending, all);
    assert!(pending.contains(&1));
    db.cleanup().await;
}

#[tokio::test]
async fn sessions_accept_truedepth_and_synthetic_but_never_simulated() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let o = org(&db.pool).await;
    let p = participant(&db.pool, o, "child-SESS-GOL").await;
    insert_session(&db.pool, "S-1", o, p, "truedepth", "green")
        .await
        .unwrap();
    insert_session(&db.pool, "S-2", o, p, "synthetic", "unreliable")
        .await
        .unwrap();
    let err = insert_session(&db.pool, "S-3", o, p, "simulated", "green")
        .await
        .unwrap_err();
    assert!(err.contains("sessions_capture_source_check"), "{err}");
    let err = insert_session(&db.pool, "S-4", o, p, "truedepth", "purple")
        .await
        .unwrap_err();
    assert!(err.contains("sessions_attention_flag_check"), "{err}");
    db.cleanup().await;
}

#[tokio::test]
async fn a_session_cannot_reference_another_orgs_participant() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let a = org(&db.pool).await;
    let b = org(&db.pool).await;
    participant(&db.pool, a, "child-SESS-GOL").await;
    let other = participant(&db.pool, b, "child-SESS-GOL").await;
    let err = insert_session(&db.pool, "S-x", a, other, "truedepth", "green")
        .await
        .unwrap_err();
    assert!(err.contains("sessions_participant_same_org_fk"), "{err}");
    db.cleanup().await;
}

#[tokio::test]
async fn participant_token_is_unique_per_org_only() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let a = org(&db.pool).await;
    let b = org(&db.pool).await;
    participant(&db.pool, a, "child-1").await;
    participant(&db.pool, b, "child-1").await; // same token, other org: allowed
    let dup = sqlx::query("INSERT INTO participants (org_id, token) VALUES ($1, 'child-1')")
        .bind(a)
        .execute(&db.pool)
        .await;
    assert!(dup.is_err());
    db.cleanup().await;
}

#[tokio::test]
async fn enrol_code_defaults_and_use_cap() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let o = org(&db.pool).await;
    let (id, max_uses, uses, ttl_days): (uuid::Uuid, i32, i32, f64) = sqlx::query_as(
        "INSERT INTO enrol_codes (org_id, code_hash) VALUES ($1, sha256('ABC-123'::bytea)) \
         RETURNING id, max_uses, uses, extract(epoch FROM expires_at - now())::float8 / 86400",
    )
    .bind(o)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!((max_uses, uses), (1, 0));
    assert!(
        (ttl_days - 7.0).abs() < 0.01,
        "default expiry is 7 days, got {ttl_days}"
    );

    sqlx::query("UPDATE enrol_codes SET uses = uses + 1 WHERE id = $1")
        .bind(id)
        .execute(&db.pool)
        .await
        .unwrap();
    let over = sqlx::query("UPDATE enrol_codes SET uses = uses + 1 WHERE id = $1")
        .bind(id)
        .execute(&db.pool)
        .await;
    assert!(over.is_err(), "uses must never exceed max_uses");
    db.cleanup().await;
}

#[tokio::test]
async fn idempotency_insert_first_reports_replay_as_zero_rows() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let insert = "INSERT INTO idempotency_keys (scope, key, request_hash) \
                  VALUES ('sessions', 'SESS-1', sha256('body'::bytea)) \
                  ON CONFLICT (scope, key) DO NOTHING RETURNING key";
    let first: Option<String> = sqlx::query_scalar(insert)
        .fetch_optional(&db.pool)
        .await
        .unwrap();
    let replay: Option<String> = sqlx::query_scalar(insert)
        .fetch_optional(&db.pool)
        .await
        .unwrap();
    assert_eq!(first.as_deref(), Some("SESS-1"));
    assert_eq!(replay, None);
    db.cleanup().await;
}

#[tokio::test]
async fn deleting_a_session_cascades_to_its_clip_scores() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let o = org(&db.pool).await;
    let p = participant(&db.pool, o, "child-SESS-GOL").await;
    insert_session(&db.pool, "S-1", o, p, "truedepth", "yellow")
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO session_clip_scores (session_id, clip_id, clip_order, score, weight, details) \
         VALUES ('S-1', 'stim_05_geopref_split_10s', 1, 0.74, 0.3, '{}')",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = 'S-1'")
        .execute(&db.pool)
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM session_clip_scores")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
    db.cleanup().await;
}

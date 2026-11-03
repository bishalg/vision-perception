//! owner: WP-X.0
//!
//! Migration 0026 (researcher toolkit, `docs/spec/modules/asd-research-toolkit/technical.md`
//! "Migration 0026"; acceptance 19): applies on a database at 0025 with sessions and on a
//! fresh one; back-fills `sessions.age_months` from whole-number `summary.ageMonths` only;
//! the new tables, columns and checks behave. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::db;
use common::TestDb;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn org(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO organizations (name, kind) VALUES ('Clinic A', 'clinic') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn user(pool: &PgPool, org: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (org_id, email, pw_hash, role) \
         VALUES ($1, $2, '$argon2id$v=19$x', 'researcher') RETURNING id",
    )
    .bind(org)
    .bind(format!("r-{}@example.org", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn session(pool: &PgPool, id: &str, org: Uuid, summary: Value) {
    let participant: Uuid =
        sqlx::query_scalar("INSERT INTO participants (org_id, token) VALUES ($1, $2) RETURNING id")
            .bind(org)
            .bind(format!("child-{id}"))
            .fetch_one(pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, org_id, participant_id, participant_token, capture_source, \
           started_at, duration_sec, schema_version, quality_verdict, valid_frame_ratio, \
           social_preference, attention_flag, summary) \
         VALUES ($1, $2, $3, $4, 'truedepth', now(), 32.5, '1.8', 'optimal', 0.93, 0.62, \
                 'green', $5)",
    )
    .bind(id)
    .bind(org)
    .bind(participant)
    .bind(format!("child-{id}"))
    .bind(summary)
    .execute(pool)
    .await
    .unwrap();
}

async fn age(pool: &PgPool, id: &str) -> Option<i32> {
    sqlx::query_scalar("SELECT age_months FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn applies_on_0025_and_backfills_whole_number_ages_only() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    db::MIGRATOR
        .run_to(25, pool)
        .await
        .expect("migrate to 0025");
    let org = org(pool).await;
    let cases: &[(&str, Value, Option<i32>)] = &[
        ("S-NUM", json!({"ageMonths": 30}), Some(30)),
        ("S-STR", json!({"ageMonths": "42"}), Some(42)),
        ("S-ZERO", json!({"ageMonths": 0}), Some(0)),
        ("S-MAX", json!({"ageMonths": 240}), Some(240)),
        ("S-FLOAT", json!({"ageMonths": 30.5}), None),
        ("S-JUNK", json!({"ageMonths": "two years"}), None),
        ("S-NEG", json!({"ageMonths": -3}), None),
        ("S-BIG", json!({"ageMonths": 241}), None),
        ("S-HUGE", json!({"ageMonths": 99_999_999_999u64}), None),
        ("S-NULL", json!({"ageMonths": null}), None),
        ("S-NONE", json!({}), None),
    ];
    for (id, summary, _) in cases {
        session(pool, id, org, summary.clone()).await;
    }

    db::MIGRATOR
        .run(pool)
        .await
        .expect("0026 applies on 0025 with sessions");
    for (id, _, want) in cases {
        assert_eq!(age(pool, id).await, *want, "{id}");
    }
    db.cleanup().await;
}

#[tokio::test]
async fn fresh_database_has_the_toolkit_schema_and_its_checks() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let org = org(pool).await;
    let other = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO organizations (name, kind) VALUES ('Clinic B', 'clinic') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let researcher = user(pool, org).await;
    session(pool, "S-1", org, json!({"ageMonths": 30})).await;

    // sessions: the age range and the research-log columns go together.
    let bad_age = sqlx::query("UPDATE sessions SET age_months = 241 WHERE id = 'S-1'")
        .execute(pool)
        .await;
    assert!(bad_age.is_err());
    let half = sqlx::query("UPDATE sessions SET research_frames_blob_key = 'k' WHERE id = 'S-1'")
        .execute(pool)
        .await;
    assert!(half.is_err(), "key without sha256 and bytes");
    sqlx::query(
        "UPDATE sessions SET research_frames_blob_key = 'k', research_frames_bytes = 10, \
           research_frames_sha256 = decode(repeat('ab', 32), 'hex') WHERE id = 'S-1'",
    )
    .execute(pool)
    .await
    .unwrap();

    // organizations, jobs, exports.
    assert!(sqlx::query(
        "UPDATE organizations SET research_frames_retention_months = 0 WHERE id = $1"
    )
    .bind(org)
    .execute(pool)
    .await
    .is_err());
    sqlx::query("UPDATE organizations SET research_frames_retention_months = 6 WHERE id = $1")
        .bind(org)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO jobs (kind, params, progress, cancel_requested_at) \
         VALUES ('analysis_run', '{}', '{\"done\": 1, \"total\": 2, \"phase\": \"x\"}', now())",
    )
    .execute(pool)
    .await
    .unwrap();
    let check: String = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname = 'exports_format_check'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(check.contains("analysis_results"), "{check}");

    // experiments: unique name per org; runs must reference the run's own org.
    let hash = vec![7u8; 32];
    let experiment: Uuid = sqlx::query_scalar(
        "INSERT INTO experiments (org_id, name, base_profile_version, profile, profile_hash, \
           created_by) VALUES ($1, 'lower ivt', '1.1.0', '{}', $2, $3) RETURNING id",
    )
    .bind(org)
    .bind(&hash)
    .bind(researcher)
    .fetch_one(pool)
    .await
    .unwrap();
    let duplicate = sqlx::query(
        "INSERT INTO experiments (org_id, name, base_profile_version, profile, profile_hash, \
           created_by) VALUES ($1, 'lower ivt', '1.1.0', '{}', $2, $3)",
    )
    .bind(org)
    .bind(&hash)
    .bind(researcher)
    .execute(pool)
    .await;
    assert!(duplicate.is_err(), "experiment-name-exists is a unique key");
    let cohort: Uuid = sqlx::query_scalar(
        "INSERT INTO cohorts (org_id, name, definition, definition_hash, created_by) \
         VALUES ($1, '24-47 months', '{}', $2, $3) RETURNING id",
    )
    .bind(org)
    .bind(&hash)
    .bind(researcher)
    .fetch_one(pool)
    .await
    .unwrap();

    let insert_run = |run_org: Uuid| {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO analysis_runs (org_id, kind, experiment_id, cohort_id, definition, \
               definition_hash, cohort_hash, profile_hash, published_profile_version, \
               baselines_version, catalog_version, core_version, contract_version, \
               pipeline_version, registry_version, seed, level_requested) \
             VALUES ($1, 'cohort_rescore', $2, $3, '{}', $4, $4, $4, '1.1.0', 'b1', 'c1', \
                     '0.1.0', '1', '1', '1.0.0', 42, 'auto') RETURNING id",
        )
        .bind(run_org)
        .bind(experiment)
        .bind(cohort)
        .bind(hash.clone())
        .fetch_one(pool)
    };
    let run = insert_run(org).await.unwrap();
    assert!(
        insert_run(other).await.is_err(),
        "a run cannot name another org's experiment"
    );
    sqlx::query("UPDATE experiments SET last_run_id = $2 WHERE id = $1")
        .bind(experiment)
        .bind(run)
        .execute(pool)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM analysis_runs WHERE id = $1")
        .bind(run)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(status, "queued");
    let bad_level =
        sqlx::query("UPDATE analysis_runs SET level_requested = 'pixels' WHERE id = $1")
            .bind(run)
            .execute(pool)
            .await;
    assert!(bad_level.is_err());
    db.cleanup().await;
}

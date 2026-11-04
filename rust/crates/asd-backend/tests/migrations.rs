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
    "analysis_runs", // WP-X.0 (0026_research_toolkit.sql)
    // WP-B.19 (0024_emr_lite.sql).
    "attachments",
    "audit_log",
    "bundle_adoption",
    "care_events",
    // WP-C.9 (0008_clinical.sql); columns and triggers in migrations_0008.rs.
    "clinical_notes",
    "clinical_review_versions",
    "clinical_reviews",
    "clinical_signatures",
    "cohorts", // WP-X.0
    "config_bundles",
    "config_documents",
    "devices",
    "encounters", // WP-B.19
    "enrol_codes",
    "erasure_requests",
    "experiments", // WP-X.0
    // WP-I.6 (0009_exports.sql).
    "export_deliveries",
    "export_items",
    "exports",
    "idempotency_keys",
    "integration_destinations",
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
    "share_links", // WP-I.13
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

/// Security audit (0015): the org-scoped newest-first indexes behind the audit log
/// and the session/worklist lists (findings 4), and the spent TOTP step (finding 7).
#[tokio::test]
async fn migration_0015_adds_list_indexes_and_the_totp_step() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    for (table, columns) in [
        ("audit_log", "(org_id, id DESC)"),
        ("sessions", "(org_id, received_at DESC, id DESC)"),
    ] {
        let defs: Vec<String> = sqlx::query_scalar(
            "SELECT indexdef FROM pg_indexes \
             WHERE schemaname = current_schema() AND tablename = $1",
        )
        .bind(table)
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert!(
            defs.iter()
                .any(|d| d.ends_with(&format!("USING btree {columns}"))),
            "{table} {columns} missing; have {defs:?}"
        );
    }
    assert_has(
        &columns(&db.pool, "users").await,
        "users",
        &["last_totp_step"],
    );
    db.cleanup().await;
}

/// The newest embedded migration, so a later package's migration does not break these tests.
fn latest() -> i64 {
    db::MIGRATOR
        .iter()
        .map(|m| m.version)
        .max()
        .expect("migrations are embedded")
}

/// Every embedded migration version from `first` on. Numbers are not contiguous (0027 is
/// reserved for WP-B.21 while 0028 exists), so a range `first..=latest()` is wrong.
fn versions_from(first: i64) -> Vec<i64> {
    db::MIGRATOR
        .iter()
        .map(|m| m.version)
        .filter(|v| *v >= first)
        .collect()
}

/// Highest successfully applied migration version.
async fn applied_max(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Config with (`true`) or without the test `ASD_DATA_KEK`.
fn config_with_kek(kek: bool) -> asd_backend::config::Config {
    let mut config = common::test_config(common::UNREACHABLE_DB_URL);
    if !kek {
        config.data_kek = None;
    }
    config
}

async fn plaintext_user(
    pool: &PgPool,
    org_id: Option<uuid::Uuid>,
    email: &str,
    secret: Option<&str>,
) -> uuid::Uuid {
    let role = if org_id.is_some() {
        "clinician"
    } else {
        "superadmin"
    };
    sqlx::query_scalar(
        "INSERT INTO users (org_id, email, pw_hash, role, totp_secret) \
         VALUES ($1, $2, 'x', $3, $4) RETURNING id",
    )
    .bind(org_id)
    .bind(email)
    .bind(role)
    .bind(secret)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The sealed seed of `id` opens and its current code verifies.
async fn assert_seed_verifies(pool: &PgPool, id: uuid::Uuid, want: &str) {
    let kek = config_with_kek(true).data_kek.expect("test KEK");
    let keyring = asd_backend::crypto::Keyring::new(kek, pool.clone());
    let secret = asd_backend::auth::totp::load_secret(pool, Some(&keyring), id)
        .await
        .unwrap()
        .expect("still enrolled");
    let code = asd_backend::clinical::reauth::totp_code(want, now_unix()).unwrap();
    assert!(
        asd_backend::auth::totp::verify_and_spend(pool, id, &secret, &code)
            .await
            .unwrap(),
        "sealed seed of {id} still verifies"
    );
}

async fn assert_plaintext_columns_dropped(pool: &PgPool) {
    assert!(!columns(pool, "users").await.contains("totp_secret"));
    assert!(!columns(pool, "review_items").await.contains("note"));
    let index: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('review_items_plain_note_idx')::text")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(index, None, "partial plaintext-note index dropped");
    let constraints: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint WHERE conname = 'users_totp_secret_one_form'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(constraints, 0, "one-form constraint dropped");
}

/// KEY-11 / acceptance 19 (fresh database): `asd-backend migrate` applies every migration
/// in order and 0022 leaves neither plaintext column; the post-migration back-fill is a
/// no-op once the column is gone.
#[tokio::test]
async fn fresh_database_migrates_to_the_end_without_plaintext_columns() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    let applied = db::migrate_with_backfill(pool, &config_with_kek(false))
        .await
        .expect("fresh migrate needs no KEK");
    let all: Vec<i64> = db::MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(applied, all, "every migration, in order");
    assert!(all.contains(&22));
    assert!(db::pending_migrations(pool).await.unwrap().is_empty());
    assert_plaintext_columns_dropped(pool).await;
    assert_eq!(
        asd_backend::auth::totp::legacy_secret_count(pool)
            .await
            .unwrap(),
        0
    );
    // Re-running is a no-op.
    assert!(db::migrate_with_backfill(pool, &config_with_kek(false))
        .await
        .unwrap()
        .is_empty());
    db.cleanup().await;
}

/// KEY-12 / acceptance 19: a database at 0019 with a plaintext seed. Without the KEK
/// `migrate` refuses naming the variable and applies nothing; with it the seed is sealed
/// first, then 0020..0022 apply and the seed still verifies.
#[tokio::test]
async fn database_at_0019_with_a_plaintext_seed_is_sealed_before_0022() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    db::MIGRATOR
        .run_to(19, pool)
        .await
        .expect("migrate to 0019");
    let org = org(pool).await;
    let user = plaintext_user(pool, Some(org), "u@example.org", Some("JBSWY3DPEHPK3PXP")).await;

    let err = db::migrate_with_backfill(pool, &config_with_kek(false))
        .await
        .unwrap_err();
    assert!(
        err.contains("1 TOTP seed(s)") && err.contains("ASD_DATA_KEK"),
        "{err}"
    );
    assert_eq!(applied_max(pool).await, 19, "nothing new applied");
    assert_eq!(
        asd_backend::auth::totp::legacy_secret_count(pool)
            .await
            .unwrap(),
        1,
        "the seed is untouched"
    );

    let applied = db::migrate_with_backfill(pool, &config_with_kek(true))
        .await
        .expect("migrate with the KEK");
    assert_eq!(applied, versions_from(20));
    assert_plaintext_columns_dropped(pool).await;
    let encrypted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'user.totp_secret_encrypt'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(encrypted, 1);
    assert_seed_verifies(pool, user, "JBSWY3DPEHPK3PXP").await;
    db.cleanup().await;
}

/// 0016 on a database at 0015 with plaintext seeds: `migrate` applies 0016, seals the
/// seeds (blank ones are cleared by the SQL) and continues to the end in one run; without
/// the KEK it stops right after 0016. A second run is a no-op.
#[tokio::test]
async fn migration_0016_seals_existing_totp_seeds() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    db::MIGRATOR
        .run_to(15, pool)
        .await
        .expect("migrate to 0015");
    let org = org(pool).await;
    let mut users = Vec::new();
    for (i, (org_id, secret)) in [
        (Some(org), Some("JBSWY3DPEHPK3PXP")),
        (None, Some(" GEZDGNBVGY3TQOJQ ")), // a superadmin: no org
        (Some(org), Some("   ")),           // blank: never enrolled
        (Some(org), None),
    ]
    .into_iter()
    .enumerate()
    {
        users.push(plaintext_user(pool, org_id, &format!("u{i}@example.org"), secret).await);
    }

    // Without the KEK: 0016 applies, then the back-fill refuses; nothing after 0016.
    let err = db::migrate_with_backfill(pool, &config_with_kek(false))
        .await
        .unwrap_err();
    assert!(
        err.contains("2 TOTP seed(s)") && err.contains("ASD_DATA_KEK"),
        "{err}"
    );
    assert_eq!(applied_max(pool).await, 16);
    assert_has(
        &columns(pool, "users").await,
        "users",
        &["totp_secret", "totp_secret_ct", "totp_secret_kek_id"],
    );
    assert_eq!(
        asd_backend::auth::totp::legacy_secret_count(pool)
            .await
            .unwrap(),
        2,
        "blank seeds are cleared by the SQL"
    );

    db::migrate_with_backfill(pool, &config_with_kek(true))
        .await
        .expect("migrate with the KEK");
    assert!(db::migrate_with_backfill(pool, &config_with_kek(true))
        .await
        .expect("second run is a no-op")
        .is_empty());
    assert_plaintext_columns_dropped(pool).await;
    let encrypted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'user.totp_secret_encrypt'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(encrypted, 2);
    assert_seed_verifies(pool, users[0], "JBSWY3DPEHPK3PXP").await;
    assert_seed_verifies(pool, users[1], "GEZDGNBVGY3TQOJQ").await;
    let kek = config_with_kek(true).data_kek.unwrap();
    let keyring = asd_backend::crypto::Keyring::new(kek, pool.clone());
    for id in [users[2], users[3]] {
        assert!(
            asd_backend::auth::totp::load_secret(pool, Some(&keyring), id)
                .await
                .unwrap()
                .is_none()
        );
    }
    db.cleanup().await;
}

/// KEY-11: applied by another tool (no `migrate` pre-step), 0022 refuses while a
/// plaintext seed remains, drops nothing, and `asd-backend migrate` then finishes the job
/// without manual SQL.
#[tokio::test]
async fn guard_0022_refuses_plaintext_seeds_and_migrate_recovers() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    db::MIGRATOR
        .run_to(21, pool)
        .await
        .expect("migrate to 0021");
    let user = plaintext_user(pool, None, "root@example.org", Some("JBSWY3DPEHPK3PXP")).await;

    // Another tool's session: a failed run keeps the migrator's advisory lock until the
    // session ends, so use (and close) a dedicated connection like a separate process.
    let mut other = pool.acquire().await.unwrap().detach();
    let err = db::MIGRATOR.run(&mut other).await.unwrap_err().to_string();
    sqlx::Connection::close(other).await.ok();
    assert!(
        err.contains(
            r#"plaintext TOTP seeds remain: run "asd-backend migrate" with ASD_DATA_KEK set"#
        ),
        "{err}"
    );
    assert_eq!(applied_max(pool).await, 21);
    assert!(columns(pool, "users").await.contains("totp_secret"));

    db::migrate_with_backfill(pool, &config_with_kek(true))
        .await
        .expect("migrate recovers");
    assert_plaintext_columns_dropped(pool).await;
    assert_seed_verifies(pool, user, "JBSWY3DPEHPK3PXP").await;
    db.cleanup().await;
}

/// KEY-11 / acceptance 19: a plaintext review note fails 0022 naming
/// `encrypt-review-notes`; nothing is dropped. Once the note is sealed (what that command
/// of the previous release does), `migrate` runs again and succeeds.
#[tokio::test]
async fn guard_0022_refuses_plaintext_review_notes_and_migrate_is_rerunnable() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    db::MIGRATOR
        .run_to(21, pool)
        .await
        .expect("migrate to 0021");
    let o = org(pool).await;
    let p = participant(pool, o, "child-SESS-GOL").await;
    insert_session(pool, "S-1", o, p, "truedepth", "green")
        .await
        .unwrap();
    let item: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO review_items (org_id, session_id, reason, note) \
         VALUES ($1, 'S-1', 'manual', 'legacy note') RETURNING id",
    )
    .bind(o)
    .fetch_one(pool)
    .await
    .unwrap();

    let err = db::migrate_with_backfill(pool, &config_with_kek(true))
        .await
        .unwrap_err();
    assert!(
        err.contains("plaintext review notes remain")
            && err.contains(r#""asd-backend admin encrypt-review-notes""#),
        "{err}"
    );
    assert!(!err.contains("legacy note"), "no note text in the error");
    assert_eq!(applied_max(pool).await, 21, "0022 rolled back");
    assert!(columns(pool, "review_items").await.contains("note"));

    // The operator's fix with the previous release: seal the note, empty the column.
    let kek = config_with_kek(true).data_kek.unwrap();
    let keyring = asd_backend::crypto::Keyring::new(kek, pool.clone());
    let aad = asd_backend::crypto::Aad::new("review_items", "note_ct", o, item);
    let sealed = keyring.encrypt_str(o, &aad, "legacy note").await.unwrap();
    sqlx::query(
        "UPDATE review_items SET note_ct = $2, note_key_id = $3, note = NULL WHERE id = $1",
    )
    .bind(item)
    .bind(&sealed.ciphertext)
    .bind(sealed.key_id)
    .execute(pool)
    .await
    .unwrap();

    let applied = db::migrate_with_backfill(pool, &config_with_kek(true))
        .await
        .expect("migrate again");
    assert_eq!(applied, versions_from(22));
    assert_plaintext_columns_dropped(pool).await;
    let (ct, key_id): (Vec<u8>, uuid::Uuid) =
        sqlx::query_as("SELECT note_ct, note_key_id FROM review_items WHERE id = $1")
            .bind(item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        keyring.decrypt_string(o, &aad, key_id, &ct).await.unwrap(),
        "legacy note"
    );
    db.cleanup().await;
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
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

/// `DATABASE_URL` with its database name replaced by the test database's.
fn test_db_url(db: &TestDb) -> String {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b.to_owned(), format!("?{q}")),
        None => (url.clone(), String::new()),
    };
    let (server, _) = base.rsplit_once('/').expect("URL has a database path");
    format!("{server}/{}{query}", db.name)
}

/// Runs `asd-backend migrate` with only `DATABASE_URL` (and `extra`) in its environment.
fn run_migrate(db: &TestDb, extra: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_asd-backend"));
    cmd.env_clear()
        .env("DATABASE_URL", test_db_url(db))
        .arg("migrate");
    for (k, v) in extra {
        cmd.env(k, v);
    }
    cmd.output().expect("run asd-backend migrate")
}

/// KEY-12 through the real binary: `asd-backend migrate` runs the seed pre-step before
/// any pending migration.
#[tokio::test]
async fn migrate_command_seals_seeds_before_pending_migrations() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    db::MIGRATOR
        .run_to(19, pool)
        .await
        .expect("migrate to 0019");
    let user = plaintext_user(pool, None, "root@example.org", Some("JBSWY3DPEHPK3PXP")).await;

    let out = run_migrate(&db, &[]);
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success(), "{log}");
    assert!(log.contains("ASD_DATA_KEK"), "{log}");
    assert!(
        !log.contains("JBSWY3DPEHPK3PXP"),
        "the seed is never logged"
    );
    assert_eq!(applied_max(pool).await, 19, "nothing new applied");

    let out = run_migrate(&db, &[("ASD_DATA_KEK", common::test_data_kek())]);
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(applied_max(pool).await, latest());
    assert_plaintext_columns_dropped(pool).await;
    assert_seed_verifies(pool, user, "JBSWY3DPEHPK3PXP").await;
    db.cleanup().await;
}

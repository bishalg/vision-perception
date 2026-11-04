//! owner: WP-L.0
//!
//! Migration 0028 (locale, `docs/spec/modules/asd-localisation/technical.md` I18N-10;
//! acceptance 18): applies on a database at 0026 with 0027 absent (reserved for WP-B.21) and
//! on a fresh one; the CHECKs refuse other locales; an org's `settings.locale` is carried
//! into `default_locale` and removed from `settings`. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::db;
use common::TestDb;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn org(pool: &PgPool, name: &str, settings: Value) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO organizations (name, kind, settings) VALUES ($1, 'clinic', $2) RETURNING id",
    )
    .bind(name)
    .bind(settings)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn user(pool: &PgPool, org: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (org_id, email, pw_hash, role) \
         VALUES ($1, $2, '$argon2id$v=19$x', 'clinician') RETURNING id",
    )
    .bind(org)
    .bind(format!("c-{}@example.org", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn org_locale(pool: &PgPool, id: Uuid) -> (String, Value) {
    sqlx::query_as("SELECT default_locale, settings FROM organizations WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[test]
fn migration_numbers_leave_0027_to_b21() {
    let versions: Vec<i64> = db::MIGRATOR.iter().map(|m| m.version).collect();
    assert!(versions.contains(&26) && versions.contains(&28));
    assert!(!versions.contains(&27), "0027 is reserved for WP-B.21");
}

#[tokio::test]
async fn applies_on_0026_and_carries_settings_locale() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let pool = &db.pool;
    db::MIGRATOR
        .run_to(26, pool)
        .await
        .expect("migrate to 0026");
    let ne = org(
        pool,
        "Clinic NE",
        json!({"locale": "ne", "audit_rate": 0.1}),
    )
    .await;
    let fr = org(pool, "Clinic FR", json!({"locale": "fr"})).await;
    let plain = org(pool, "Clinic", json!({"audit_rate": 0.2})).await;

    db::MIGRATOR
        .run(pool)
        .await
        .expect("0028 applies on 0026 without 0027");

    assert_eq!(
        org_locale(pool, ne).await,
        ("ne".to_string(), json!({"audit_rate": 0.1}))
    );
    // An unknown value is not carried (the CHECK would refuse it) but the key is removed.
    assert_eq!(org_locale(pool, fr).await, ("en".to_string(), json!({})));
    assert_eq!(
        org_locale(pool, plain).await,
        ("en".to_string(), json!({"audit_rate": 0.2}))
    );
    db.cleanup().await;
}

#[tokio::test]
async fn fresh_database_has_the_locale_columns_and_checks() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let o = org(pool, "Clinic A", json!({})).await;
    let u = user(pool, o).await;

    let locale: Option<String> = sqlx::query_scalar("SELECT locale FROM users WHERE id = $1")
        .bind(u)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(locale, None);
    assert_eq!(org_locale(pool, o).await.0, "en");

    for (sql, id) in [
        ("UPDATE users SET locale = $2 WHERE id = $1", u),
        (
            "UPDATE organizations SET default_locale = $2 WHERE id = $1",
            o,
        ),
    ] {
        let fr = sqlx::query(sql).bind(id).bind("fr").execute(pool).await;
        assert!(fr.is_err(), "{sql}: 'fr' must be refused");
        sqlx::query(sql)
            .bind(id)
            .bind("ne")
            .execute(pool)
            .await
            .expect("ne accepted");
    }
    let null_org = sqlx::query("UPDATE organizations SET default_locale = NULL WHERE id = $1")
        .bind(o)
        .execute(pool)
        .await;
    assert!(null_org.is_err(), "default_locale is NOT NULL");
    sqlx::query("UPDATE users SET locale = NULL WHERE id = $1")
        .bind(u)
        .execute(pool)
        .await
        .expect("a user may clear the override");
    db.cleanup().await;
}

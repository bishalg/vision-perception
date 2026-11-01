//! owner: WP-B.15
//!
//! KEY-8..10: the keyring reads under the active and any previous KEK, writes under the
//! active one; `asd-backend admin rotate-kek` re-wraps every org data key and re-seals
//! every user secret still under a previous KEK, compare-and-set, idempotent. Every check
//! of the database state uses a *fresh* `Keyring` (phase-5 trap 3: unwrapped DEKs are
//! cached per process). Database tests need `DATABASE_URL`; skipped otherwise.

mod common;

use std::sync::Arc;

use asd_backend::admin::{self, AdminCommand, AdminError};
use asd_backend::auth::totp;
use asd_backend::clinical::reauth::totp_code;
use asd_backend::config::{Config, ConfigError};
use asd_backend::crypto::rotate::{self, RotateOptions, USER_SECRET_COLUMNS};
use asd_backend::crypto::{Aad, CryptoError, Kek, Keyring, Sealed};
use asd_contract::api_v1::Role;
use base64::Engine;
use common::auth::{from_ip, json, seed_org, seed_user, PASSWORD};
use common::{send, TestDb};
use http::{Method, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

const NOTE: &str = "Brief distress in clip 3; engaged otherwise. नेपाल";
const SEED_1: &str = "JBSWY3DPEHPK3PXP";
const SEED_2: &str = "KRSXG5CTMVRXEZLU";

fn random_kek_b64() -> String {
    let mut k = [0u8; 32];
    k[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    k[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    base64::engine::general_purpose::STANDARD.encode(k)
}

fn kek(b64: &str) -> Kek {
    Kek::from_base64(b64).unwrap()
}

/// A database written entirely under KEK `A`: two orgs (the first with a retired and an
/// active DEK, so the blind-index key comes from the retired one), two users with sealed
/// TOTP seeds and one without.
struct Fx {
    db: TestDb,
    a: String,
    b: String,
    org1: Uuid,
    /// (org, aad, sealed) of each note written under A.
    notes: Vec<(Uuid, Aad, Sealed)>,
    users: Vec<(Uuid, &'static str)>,
    plain_user: Uuid,
    blind_index: [u8; 32],
}

async fn setup() -> Option<Fx> {
    let db = TestDb::migrated().await?;
    let a = random_kek_b64();
    let b = random_kek_b64();
    let org1 = seed_org(&db.pool, "Clinic One").await;
    let org2 = seed_org(&db.pool, "Clinic Two").await;
    let mut notes = Vec::new();

    let ring_a = Keyring::new(kek(&a), db.pool.clone());
    let aad = Aad::new("clinical_notes", "body_ct", org1, Uuid::new_v4());
    notes.push((
        org1,
        aad.clone(),
        ring_a.encrypt_str(org1, &aad, NOTE).await.unwrap(),
    ));
    let blind_index = ring_a.mrn_blind_index(org1, "TUTH", "12 34").await.unwrap();
    // Retire org1's first key; a fresh keyring (no cached active key) creates the next.
    sqlx::query("UPDATE org_data_keys SET status = 'retired' WHERE org_id = $1")
        .bind(org1)
        .execute(&db.pool)
        .await
        .unwrap();
    let ring_a = Keyring::new(kek(&a), db.pool.clone());
    for org in [org1, org2] {
        let aad = Aad::new("clinical_notes", "body_ct", org, Uuid::new_v4());
        notes.push((
            org,
            aad.clone(),
            ring_a.encrypt_str(org, &aad, NOTE).await.unwrap(),
        ));
    }
    assert_eq!(dek_count(&db.pool).await, 3);

    let u1 = seed_user(&db.pool, Some(org1), Role::Owner, "one@example.org").await;
    let u2 = seed_user(&db.pool, None, Role::Superadmin, "two@example.org").await;
    let plain_user = seed_user(&db.pool, Some(org2), Role::Owner, "three@example.org").await;
    totp::set_secret(&db.pool, &ring_a, u1, Some(SEED_1))
        .await
        .unwrap();
    totp::set_secret(&db.pool, &ring_a, u2, Some(SEED_2))
        .await
        .unwrap();

    Some(Fx {
        db,
        a,
        b,
        org1,
        notes,
        users: vec![(u1, SEED_1), (u2, SEED_2)],
        plain_user,
        blind_index,
    })
}

impl Fx {
    /// Active `B`, previous `A`: the mid-rotation configuration.
    fn ring_ba(&self) -> Keyring {
        Keyring::with_previous(kek(&self.b), vec![kek(&self.a)], self.db.pool.clone())
    }

    fn ring_b(&self) -> Keyring {
        Keyring::new(kek(&self.b), self.db.pool.clone())
    }
}

async fn dek_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM org_data_keys")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn dek_kek_ids(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT kek_id FROM org_data_keys ORDER BY key_id")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn user_kek_ids(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT totp_secret_kek_id FROM users WHERE totp_secret_ct IS NOT NULL ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn seed_of(pool: &PgPool, ring: &Keyring, user: Uuid) -> Result<String, String> {
    match totp::load_secret(pool, Some(ring), user).await {
        Ok(Some(_)) => {
            // `TotpSecret` hides its value; prove it by opening the row directly.
            let (ct, kek_id): (Vec<u8>, String) = sqlx::query_as(
                "SELECT totp_secret_ct, totp_secret_kek_id FROM users WHERE id = $1",
            )
            .bind(user)
            .fetch_one(pool)
            .await
            .unwrap();
            let plain = ring
                .open_user_secret(&totp::secret_aad(user), &kek_id, &ct)
                .map_err(|e| format!("{e:?}"))?;
            Ok(String::from_utf8(plain.to_vec()).unwrap())
        }
        Ok(None) => Err("not enrolled".into()),
        Err(e) => Err(format!("{e:?}")),
    }
}

async fn assert_everything_readable(f: &Fx, ring: &Keyring) {
    for (org, aad, sealed) in &f.notes {
        let plain = ring
            .decrypt_string(*org, aad, sealed.key_id, &sealed.ciphertext)
            .await
            .unwrap_or_else(|e| panic!("note of {org}: {e:?}"));
        assert_eq!(plain, NOTE);
    }
    for (user, seed) in &f.users {
        assert_eq!(seed_of(&f.db.pool, ring, *user).await.as_deref(), Ok(*seed));
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn opts() -> RotateOptions {
    RotateOptions {
        batch: rotate::DEFAULT_BATCH,
        dry_run: false,
    }
}

// ---- KEY-8: reads under previous keys, writes under the active one ------------------

#[tokio::test]
async fn previous_kek_reads_and_active_kek_writes() {
    let Some(f) = setup().await else { return };
    let ring = f.ring_ba();
    assert_eq!(ring.kek_id(), kek(&f.b).id());
    assert_everything_readable(&f, &ring).await;
    assert_eq!(
        ring.mrn_blind_index(f.org1, "TUTH", "12 34").await.unwrap(),
        f.blind_index
    );

    // A new org's first DEK is wrapped under B.
    let org3 = seed_org(&f.db.pool, "Clinic Three").await;
    let aad = Aad::new("clinical_notes", "body_ct", org3, Uuid::new_v4());
    ring.encrypt_str(org3, &aad, NOTE).await.unwrap();
    let new_kek: String = sqlx::query_scalar("SELECT kek_id FROM org_data_keys WHERE org_id = $1")
        .bind(org3)
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(new_kek, kek(&f.b).id());
    // A new user secret is sealed under B.
    totp::set_secret(&f.db.pool, &ring, f.plain_user, Some(SEED_1))
        .await
        .unwrap();
    let id: String = sqlx::query_scalar("SELECT totp_secret_kek_id FROM users WHERE id = $1")
        .bind(f.plain_user)
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(id, kek(&f.b).id());
    assert_eq!(
        seed_of(&f.db.pool, &f.ring_b(), f.plain_user)
            .await
            .as_deref(),
        Ok(SEED_1)
    );
    f.db.cleanup().await;
}

/// Phase-5 trap 2: `open_user_secret` selects the KEK by the stored id instead of
/// refusing every id that is not the active one.
#[tokio::test]
async fn open_user_secret_selects_the_kek_by_stored_id() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy(common::UNREACHABLE_DB_URL)
        .unwrap();
    let (a, b, c) = (random_kek_b64(), random_kek_b64(), random_kek_b64());
    let user = Uuid::new_v4();
    let aad = totp::secret_aad(user);
    let ring_a = Keyring::new(kek(&a), pool.clone());
    let ct = ring_a.seal_user_secret(&aad, SEED_1.as_bytes()).unwrap();
    let ring_ba = Keyring::with_previous(kek(&b), vec![kek(&a)], pool.clone());
    assert_eq!(
        ring_ba
            .open_user_secret(&aad, kek(&a).id(), &ct)
            .unwrap()
            .as_slice(),
        SEED_1.as_bytes()
    );
    // The stored id decides: claiming B for an A ciphertext fails authentication.
    assert!(matches!(
        ring_ba.open_user_secret(&aad, kek(&b).id(), &ct),
        Err(CryptoError::Decrypt)
    ));
    // A KEK that is not configured at all: KeyUnwrap (503).
    let ring_cb = Keyring::with_previous(kek(&c), vec![kek(&b)], pool);
    assert!(matches!(
        ring_cb.open_user_secret(&aad, kek(&a).id(), &ct),
        Err(CryptoError::KeyUnwrap)
    ));
}

#[tokio::test]
async fn only_the_new_kek_before_rotation_is_key_unwrap() {
    let Some(f) = setup().await else { return };
    let ring = f.ring_b();
    let (org, aad, sealed) = &f.notes[0];
    let err = ring
        .decrypt(*org, aad, sealed.key_id, &sealed.ciphertext)
        .await
        .unwrap_err();
    assert!(matches!(err, CryptoError::KeyUnwrap), "{err:?}");
    let (user, _) = f.users[0];
    let err = seed_of(&f.db.pool, &ring, user).await.unwrap_err();
    assert!(
        err.contains("Unavailable") || err.contains("ServiceUnavailable"),
        "{err}"
    );
    f.db.cleanup().await;
}

// ---- KEY-9: rotate-kek --------------------------------------------------------------

#[tokio::test]
async fn rotation_rewrites_every_row_and_a_second_run_changes_nothing() {
    let Some(f) = setup().await else { return };
    let report = rotate::rotate_kek(&f.db.pool, &f.ring_ba(), opts())
        .await
        .unwrap();
    assert_eq!(report.rewrapped_deks, 3);
    assert_eq!(report.resealed_users, 2);
    assert_eq!(report.remaining, 0);
    assert!(!report.dry_run);
    let b_id = kek(&f.b).id().to_owned();
    assert!(dek_kek_ids(&f.db.pool).await.iter().all(|id| *id == b_id));
    assert_eq!(
        user_kek_ids(&f.db.pool).await,
        vec![b_id.clone(), b_id.clone()]
    );
    // The user without a seed stays without one.
    let none: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT totp_secret_ct FROM users WHERE id = $1")
            .bind(f.plain_user)
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert!(none.is_none());

    // Only B from now on: the same DEKs (same key ids, same ciphertexts) and seeds open,
    // and the blind index (derived from the retired first DEK) is byte-identical.
    let ring = f.ring_b();
    assert_everything_readable(&f, &ring).await;
    assert_eq!(
        ring.mrn_blind_index(f.org1, "TUTH", "12 34").await.unwrap(),
        f.blind_index
    );
    assert_eq!(dek_count(&f.db.pool).await, 3, "no DEK was replaced");

    // One audit row with counts and fingerprints only.
    let (org, after): (Option<Uuid>, serde_json::Value) =
        sqlx::query_as("SELECT org_id, after FROM audit_log WHERE action = 'crypto.kek_rotated'")
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert_eq!(org, None);
    assert_eq!(after["rewrapped_deks"], 3);
    assert_eq!(after["resealed_users"], 2);
    assert_eq!(after["remaining"], 0);
    assert_eq!(after["active_kek"], b_id);
    assert_eq!(after["previous_keks"], serde_json::json!([kek(&f.a).id()]));
    let text = after.to_string();
    for secret in [f.a.as_str(), f.b.as_str(), SEED_1, SEED_2] {
        assert!(!text.contains(secret), "{text}");
    }

    // Idempotent.
    let again = rotate::rotate_kek(&f.db.pool, &f.ring_ba(), opts())
        .await
        .unwrap();
    assert_eq!(
        (again.rewrapped_deks, again.resealed_users, again.remaining),
        (0, 0, 0)
    );
    assert_everything_readable(&f, &f.ring_b()).await;
    f.db.cleanup().await;
}

#[tokio::test]
async fn small_batches_cover_every_row() {
    let Some(f) = setup().await else { return };
    let report = rotate::rotate_kek(
        &f.db.pool,
        &f.ring_ba(),
        RotateOptions {
            batch: 1,
            dry_run: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        (
            report.rewrapped_deks,
            report.resealed_users,
            report.remaining
        ),
        (3, 2, 0)
    );
    assert_everything_readable(&f, &f.ring_b()).await;
    f.db.cleanup().await;
}

#[tokio::test]
async fn dry_run_counts_and_changes_nothing() {
    let Some(f) = setup().await else { return };
    let before_deks = dek_kek_ids(&f.db.pool).await;
    let report = rotate::rotate_kek(
        &f.db.pool,
        &f.ring_ba(),
        RotateOptions {
            batch: 2,
            dry_run: true,
        },
    )
    .await
    .unwrap();
    assert!(report.dry_run);
    assert_eq!(
        (
            report.rewrapped_deks,
            report.resealed_users,
            report.remaining
        ),
        (3, 2, 0)
    );
    assert_eq!(dek_kek_ids(&f.db.pool).await, before_deks);
    let a_id = kek(&f.a).id().to_owned();
    assert_eq!(user_kek_ids(&f.db.pool).await, vec![a_id.clone(), a_id]);
    let audits: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'crypto.kek_rotated'")
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert_eq!(audits, 0);
    f.db.cleanup().await;
}

/// KEY-9 compare-and-set: an update whose expected old KEK no longer matches (a
/// concurrent writer re-sealed the row under B first) changes nothing.
#[tokio::test]
async fn compare_and_set_never_overwrites_a_concurrent_rewrite() {
    let Some(f) = setup().await else { return };
    let (user, _) = f.users[0];
    let a_id = kek(&f.a).id().to_owned();
    let b_id = kek(&f.b).id().to_owned();
    let column = &USER_SECRET_COLUMNS[0];
    // What rotation computed from the row it read under A ...
    let (ct_a, _): (Vec<u8>, String) =
        sqlx::query_as("SELECT totp_secret_ct, totp_secret_kek_id FROM users WHERE id = $1")
            .bind(user)
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    let ring = f.ring_ba();
    let stale = ring
        .reseal_user_secret(&(column.aad)(user), &a_id, &ct_a)
        .unwrap();
    // ... while a concurrent writer replaced the seed under B.
    totp::set_secret(&f.db.pool, &ring, user, Some(SEED_2))
        .await
        .unwrap();
    let mut conn = f.db.pool.acquire().await.unwrap();
    let changed = rotate::reseal_user_secret_cas(&mut conn, column, user, &a_id, &stale, &b_id)
        .await
        .unwrap();
    assert!(!changed);
    drop(conn);
    assert_eq!(
        seed_of(&f.db.pool, &f.ring_b(), user).await.as_deref(),
        Ok(SEED_2),
        "the concurrent writer's seed survives"
    );

    // The same for a DEK: a CAS against the wrong old KEK id leaves the row alone.
    let (key_id, org, wrapped): (Uuid, Uuid, Vec<u8>) = sqlx::query_as(
        "SELECT key_id, org_id, wrapped_dek FROM org_data_keys ORDER BY key_id LIMIT 1",
    )
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    let rewrapped = ring.rewrap_dek(org, key_id, &wrapped, &a_id).unwrap();
    let mut conn = f.db.pool.acquire().await.unwrap();
    let changed =
        rotate::rewrap_dek_cas(&mut conn, key_id, "kek-0000000000000000", &rewrapped, &b_id)
            .await
            .unwrap();
    assert!(!changed);
    let changed = rotate::rewrap_dek_cas(&mut conn, key_id, &a_id, &rewrapped, &b_id)
        .await
        .unwrap();
    assert!(changed);
    // `cleanup` closes the pool, which waits for every checked-out connection.
    drop(conn);
    f.db.cleanup().await;
}

/// A row locked by another transaction is skipped (`SKIP LOCKED`), reported as
/// remaining, and picked up by the next run after that writer committed.
#[tokio::test]
async fn locked_rows_are_skipped_and_reported() {
    let Some(f) = setup().await else { return };
    let (user, _) = f.users[0];
    let mut tx = f.db.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    let report = rotate::rotate_kek(&f.db.pool, &f.ring_ba(), opts())
        .await
        .unwrap();
    assert_eq!(
        (
            report.rewrapped_deks,
            report.resealed_users,
            report.remaining
        ),
        (3, 1, 1)
    );
    tx.commit().await.unwrap();
    let report = rotate::rotate_kek(&f.db.pool, &f.ring_ba(), opts())
        .await
        .unwrap();
    assert_eq!(
        (
            report.rewrapped_deks,
            report.resealed_users,
            report.remaining
        ),
        (0, 1, 0)
    );
    assert_everything_readable(&f, &f.ring_b()).await;
    f.db.cleanup().await;
}

fn config_with(active: Option<&str>, previous: Option<&str>) -> Result<Config, ConfigError> {
    let (active, previous) = (active.map(str::to_owned), previous.map(str::to_owned));
    let jwt = common::test_jwt_key().to_owned();
    Config::from_lookup(move |key| match key {
        "DATABASE_URL" => Some(common::UNREACHABLE_DB_URL.to_owned()),
        "ASD_BLOB_BACKEND" => Some("memory".to_owned()),
        "ASD_JWT_SIGNING_KEY" => Some(jwt.clone()),
        "ASD_DATA_KEK" => active.clone(),
        "ASD_DATA_KEK_PREVIOUS" => previous.clone(),
        _ => None,
    })
}

/// A row under a KEK that is not configured stays as it is, counts as remaining, and the
/// CLI exits 3 (after printing the report).
#[tokio::test]
async fn rows_under_an_unknown_kek_are_remaining_and_exit_3() {
    let Some(f) = setup().await else { return };
    let c = random_kek_b64();
    let stranger = seed_user(&f.db.pool, None, Role::Superadmin, "c@example.org").await;
    let ring_c = Keyring::new(kek(&c), f.db.pool.clone());
    totp::set_secret(&f.db.pool, &ring_c, stranger, Some(SEED_1))
        .await
        .unwrap();

    let config = config_with(Some(&f.b), Some(&f.a)).unwrap();
    let err = admin::rotate_kek_cli(&f.db.pool, &config, rotate::DEFAULT_BATCH, false)
        .await
        .unwrap_err();
    assert_eq!(err.exit_code(), 3, "{err}");
    let AdminError::RotationIncomplete(report) = &err else {
        panic!("{err:?}")
    };
    assert_eq!(
        (
            report.rewrapped_deks,
            report.resealed_users,
            report.remaining
        ),
        (3, 2, 1)
    );
    let c_id: String = sqlx::query_scalar("SELECT totp_secret_kek_id FROM users WHERE id = $1")
        .bind(stranger)
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(c_id, kek(&c).id(), "untouched");
    assert!(!err.to_string().contains(&c));

    // Without that row, the CLI succeeds and prints the JSON report.
    sqlx::query("UPDATE users SET totp_secret_ct = NULL, totp_secret_kek_id = NULL WHERE id = $1")
        .bind(stranger)
        .execute(&f.db.pool)
        .await
        .unwrap();
    let out = admin::rotate_kek_cli(&f.db.pool, &config, rotate::DEFAULT_BATCH, false)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["remaining"], 0);
    assert_eq!(v["rewrapped_deks"], 0);

    // Without ASD_DATA_KEK the command refuses (exit 2).
    let none = config_with(None, None).unwrap();
    let err = admin::rotate_kek_cli(&f.db.pool, &none, 500, false)
        .await
        .unwrap_err();
    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("ASD_DATA_KEK"));
    f.db.cleanup().await;
}

/// A pending TOTP enrolment (0020) is a KEK-sealed secret too: it is counted while under a
/// previous KEK, re-sealed, and still confirms once only the new KEK is configured.
#[tokio::test]
async fn pending_totp_seed_is_resealed_and_confirms_after_the_old_kek_is_gone() {
    let Some(f) = setup().await else { return };
    let user = f.plain_user;
    let ring_a = Keyring::new(kek(&f.a), f.db.pool.clone());
    let sealed = ring_a
        .seal_user_secret(&totp::pending_aad(user), SEED_1.as_bytes())
        .unwrap();
    sqlx::query(
        "UPDATE users SET totp_pending_ct = $2, totp_pending_kek_id = $3,
                totp_pending_expires_at = now() + interval '10 minutes'
         WHERE id = $1",
    )
    .bind(user)
    .bind(&sealed)
    .bind(ring_a.kek_id())
    .execute(&f.db.pool)
    .await
    .unwrap();
    assert!(USER_SECRET_COLUMNS
        .iter()
        .any(|c| c.ct == "totp_pending_ct" && c.kek == "totp_pending_kek_id"));

    // Without A configured nothing can move, and the pending seed counts as remaining.
    let stuck = rotate::rotate_kek(&f.db.pool, &f.ring_b(), opts())
        .await
        .unwrap();
    assert_eq!(
        (stuck.rewrapped_deks, stuck.resealed_users, stuck.remaining),
        (0, 0, 6)
    );

    let report = rotate::rotate_kek(&f.db.pool, &f.ring_ba(), opts())
        .await
        .unwrap();
    assert_eq!(
        (
            report.rewrapped_deks,
            report.resealed_users,
            report.remaining
        ),
        (3, 3, 0)
    );
    let pending_kek: String =
        sqlx::query_scalar("SELECT totp_pending_kek_id FROM users WHERE id = $1")
            .bind(user)
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert_eq!(pending_kek, kek(&f.b).id());

    // Only B from now on: the pending seed still confirms.
    let ring = f.ring_b();
    let code = totp_code(SEED_1, now_unix()).unwrap();
    let outcome = totp::confirm_enrolment(
        &f.db.pool,
        &ring,
        user,
        &code,
        asd_backend::audit::AuditEvent::system("user.totp_enrolled", "user", user),
    )
    .await
    .unwrap();
    assert_eq!(outcome, totp::ConfirmOutcome::Enrolled);
    assert_eq!(
        seed_of(&f.db.pool, &ring, user).await.as_deref(),
        Ok(SEED_1)
    );
    f.db.cleanup().await;
}

/// A DEK whose wrapped bytes are damaged cannot be re-wrapped: it stays as it is, counts as
/// remaining and the CLI exits 3, while every other row is rotated.
#[tokio::test]
async fn a_corrupt_dek_is_remaining_exit_3_and_the_rest_rotates() {
    let Some(f) = setup().await else { return };
    let corrupt: Uuid = sqlx::query_scalar(
        "UPDATE org_data_keys
         SET wrapped_dek = set_byte(wrapped_dek, 20, 255 - get_byte(wrapped_dek, 20))
         WHERE key_id = (SELECT key_id FROM org_data_keys ORDER BY key_id LIMIT 1)
         RETURNING key_id",
    )
    .fetch_one(&f.db.pool)
    .await
    .unwrap();

    let config = config_with(Some(&f.b), Some(&f.a)).unwrap();
    let err = admin::rotate_kek_cli(&f.db.pool, &config, rotate::DEFAULT_BATCH, false)
        .await
        .unwrap_err();
    assert_eq!(err.exit_code(), 3, "{err}");
    let AdminError::RotationIncomplete(report) = &err else {
        panic!("{err:?}")
    };
    assert_eq!(
        (
            report.rewrapped_deks,
            report.resealed_users,
            report.remaining
        ),
        (2, 2, 1)
    );
    let (a_id, b_id) = (kek(&f.a).id().to_owned(), kek(&f.b).id().to_owned());
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT key_id, kek_id FROM org_data_keys ORDER BY key_id")
            .fetch_all(&f.db.pool)
            .await
            .unwrap();
    for (key_id, kek_id) in rows {
        let expected = if key_id == corrupt { &a_id } else { &b_id };
        assert_eq!(&kek_id, expected, "{key_id}");
    }
    assert_eq!(user_kek_ids(&f.db.pool).await, vec![b_id.clone(), b_id]);
    for (user, seed) in &f.users {
        assert_eq!(
            seed_of(&f.db.pool, &f.ring_b(), *user).await.as_deref(),
            Ok(*seed)
        );
    }
    f.db.cleanup().await;
}

// ---- KEY-8 configuration ------------------------------------------------------------

#[test]
fn previous_keks_are_parsed_validated_and_never_printed() {
    let (a, b, c) = (random_kek_b64(), random_kek_b64(), random_kek_b64());
    let cfg = config_with(Some(&c), Some(&format!("{a}, {b}"))).unwrap();
    let ids: Vec<&str> = cfg.data_kek_previous.iter().map(Kek::id).collect();
    assert_eq!(ids, vec![kek(&a).id(), kek(&b).id()]);
    let dbg = format!("{cfg:?}");
    for secret in [&a, &b, &c] {
        assert!(!dbg.contains(secret.as_str()), "{dbg}");
    }
    assert!(config_with(Some(&c), None)
        .unwrap()
        .data_kek_previous
        .is_empty());

    let short = base64::engine::general_purpose::STANDARD.encode([7u8; 16]);
    for (active, previous) in [
        (Some(c.as_str()), c.clone()),              // equal to the active key
        (Some(c.as_str()), format!("{a},{a}")),     // duplicate
        (Some(c.as_str()), format!("{a},{short}")), // not 32 bytes
        (Some(c.as_str()), format!("{a},not base64!")), // not base64
        (Some(c.as_str()), format!("{a},,{b}")),    // empty entry
        (None, a.clone()),                          // previous without an active key
    ] {
        match config_with(active, Some(&previous)).unwrap_err() {
            ConfigError::Invalid { name, reason } => {
                assert_eq!(name, "ASD_DATA_KEK_PREVIOUS", "{previous}");
                for secret in [&a, &b, &c, &short] {
                    assert!(!reason.contains(secret.as_str()), "{reason}");
                }
            }
            other => panic!("{previous}: {other:?}"),
        }
    }
}

#[test]
fn rotate_kek_command_parses() {
    let args = |s: &[&str]| s.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        admin::parse(&args(&["rotate-kek"])).unwrap(),
        AdminCommand::RotateKek {
            batch: rotate::DEFAULT_BATCH,
            dry_run: false
        }
    );
    assert_eq!(
        admin::parse(&args(&["rotate-kek", "--batch", "50", "--dry-run"])).unwrap(),
        AdminCommand::RotateKek {
            batch: 50,
            dry_run: true
        }
    );
    for bad in [
        &["rotate-kek", "--batch", "0"][..],
        &["rotate-kek", "--batch", "x"],
        &["rotate-kek", "--force"],
        &["rotate-kek", "--dry-run=yes"],
        &["create-org", "--name", "x", "--kind", "clinic", "--dry-run"],
    ] {
        assert_eq!(
            admin::parse(&args(bad)).unwrap_err().exit_code(),
            2,
            "{bad:?}"
        );
    }
}

// ---- acceptance 18: the API mid-rotation and after ----------------------------------

#[tokio::test]
async fn totp_login_works_mid_rotation_and_after() {
    let Some(f) = setup().await else { return };
    let (user, seed) = f.users[0];
    let config = config_with(Some(&f.b), Some(&f.a)).unwrap();
    let mut state = common::state_with_pool(f.db.pool.clone());
    // `state_with_pool` uses the process KEK; swap in this test's keyring the same way
    // `AppState::new` builds it from the configuration above.
    state.keyring = Some(Arc::new(Keyring::with_previous(
        config.data_kek.clone().unwrap(),
        config.data_kek_previous.clone(),
        f.db.pool.clone(),
    )));
    let login = |state: asd_backend::app::AppState, ip: &'static str| async move {
        let code = totp_code(
            seed,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
        .unwrap();
        send(
            common::router(state),
            from_ip(
                json(
                    Method::POST,
                    "/v1/auth/login",
                    &serde_json::json!({
                        "email": "one@example.org", "password": PASSWORD, "totp_code": code
                    }),
                    None,
                ),
                ip,
            ),
        )
        .await
    };
    let (status, _, body) = login(state.clone(), "198.51.100.20").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Only B before the rotation: 503 for the row still under A.
    let mut only_b = state.clone();
    only_b.keyring = Some(Arc::new(f.ring_b()));
    // Spent step: a second login in the same step is refused as a replay before the
    // seed matters, so check the seed path directly.
    let err = totp::load_secret(&f.db.pool, only_b.keyring.as_deref(), user)
        .await
        .unwrap_err();
    let problem = err.to_problem();
    assert_eq!(problem.status, 503);
    assert!(problem.type_uri.ends_with("/clinical-crypto-unavailable"));

    rotate::rotate_kek(&f.db.pool, state.keyring.as_deref().unwrap(), opts())
        .await
        .unwrap();
    assert_everything_readable(&f, only_b.keyring.as_deref().unwrap()).await;
    f.db.cleanup().await;
}

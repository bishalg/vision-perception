//! owner: WP-C.9
//!
//! KEY-1..6: application-level envelope encryption. AES-256-GCM with a per-org data key
//! (DEK) wrapped by `ASD_DATA_KEK`; AAD `"{table}:{column}:{org_id}:{row_id}"`; ciphertext
//! layout `version(1) || nonce(12) || ciphertext || tag(16)`; MRN blind index (KEY-4); a
//! missing KEK leaves the keyring absent and the clinical extractor answering 503.
//! Database tests need `DATABASE_URL`; skipped otherwise.

mod common;

use std::sync::Arc;

use asd_backend::app::AppState;
use asd_backend::blob::MemoryBlobStore;
use asd_backend::config::{Config, ConfigError};
use asd_backend::crypto::blind_index::normalize;
use asd_backend::crypto::{Aad, ClinicalCrypto, CryptoError, Kek, Keyring};
use asd_backend::db;
use axum::routing::get;
use axum::Router;
use base64::Engine;
use common::{send, TestDb};
use http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

async fn org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organizations (name, kind) VALUES ($1, 'clinic') RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

struct Fx {
    db: TestDb,
    keyring: Keyring,
    kek_b64: String,
    a: Uuid,
    b: Uuid,
}

async fn setup() -> Option<Fx> {
    let db = TestDb::migrated().await?;
    let kek_b64 = random_kek_b64();
    let keyring = Keyring::new(Kek::from_base64(&kek_b64).unwrap(), db.pool.clone());
    let a = org(&db.pool, "Clinic A").await;
    let b = org(&db.pool, "Clinic B").await;
    Some(Fx {
        db,
        keyring,
        kek_b64,
        a,
        b,
    })
}

fn random_kek_b64() -> String {
    let mut k = [0u8; 32];
    k[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    k[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    base64::engine::general_purpose::STANDARD.encode(k)
}

const NOTE: &[u8] = "Child engaged well; brief distress in clip 3. नेपाल".as_bytes();

#[tokio::test]
async fn round_trip_and_layout() {
    let Some(f) = setup().await else { return };
    let row = Uuid::new_v4();
    let aad = Aad::new("clinical_notes", "body_ct", f.a, row);
    let sealed = f.keyring.encrypt(f.a, &aad, NOTE).await.unwrap();
    assert_eq!(sealed.ciphertext[0], 1, "version byte");
    assert_eq!(sealed.ciphertext.len(), 1 + 12 + NOTE.len() + 16);
    assert!(!sealed
        .ciphertext
        .windows(NOTE.len().min(8))
        .any(|w| w == &NOTE[..8]));
    let plain = f
        .keyring
        .decrypt(f.a, &aad, sealed.key_id, &sealed.ciphertext)
        .await
        .unwrap();
    assert_eq!(plain, NOTE);
    assert_eq!(
        f.keyring
            .decrypt_string(f.a, &aad, sealed.key_id, &sealed.ciphertext)
            .await
            .unwrap(),
        std::str::from_utf8(NOTE).unwrap()
    );

    // Random nonce: the same plaintext never gives the same bytes; same key id.
    let again = f.keyring.encrypt(f.a, &aad, NOTE).await.unwrap();
    assert_ne!(again.ciphertext, sealed.ciphertext);
    assert_eq!(again.key_id, sealed.key_id);
    assert_eq!(
        aad.to_string(),
        format!("clinical_notes:body_ct:{}:{row}", f.a)
    );
    f.db.cleanup().await;
}

#[tokio::test]
async fn aad_swap_fails() {
    let Some(f) = setup().await else { return };
    let row = Uuid::new_v4();
    let aad = Aad::new("clinical_notes", "body_ct", f.a, row);
    let s = f.keyring.encrypt(f.a, &aad, NOTE).await.unwrap();

    for (what, wrong) in [
        (
            "row id",
            Aad::new("clinical_notes", "body_ct", f.a, Uuid::new_v4()),
        ),
        ("column", Aad::new("clinical_notes", "other_ct", f.a, row)),
        ("table", Aad::new("referral_letters", "body_ct", f.a, row)),
        (
            "org id in AAD",
            Aad::new("clinical_notes", "body_ct", f.b, row),
        ),
    ] {
        let err = f
            .keyring
            .decrypt(f.a, &wrong, s.key_id, &s.ciphertext)
            .await
            .unwrap_err();
        assert!(matches!(err, CryptoError::Decrypt), "{what}: {err:?}");
    }
    // Copied to another org's row: org B cannot use org A's key at all.
    let err = f
        .keyring
        .decrypt(
            f.b,
            &Aad::new("clinical_notes", "body_ct", f.b, row),
            s.key_id,
            &s.ciphertext,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, CryptoError::KeyNotFound), "{err:?}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn tampering_fails() {
    let Some(f) = setup().await else { return };
    let aad = Aad::new("patient_refs", "mrn_ct", f.a, Uuid::new_v4());
    let s = f.keyring.encrypt(f.a, &aad, b"MRN-0042").await.unwrap();
    let n = s.ciphertext.len();
    for (what, idx) in [("tag", n - 1), ("ciphertext", 13), ("nonce", 1)] {
        let mut bad = s.ciphertext.clone();
        bad[idx] ^= 0x01;
        let err = f
            .keyring
            .decrypt(f.a, &aad, s.key_id, &bad)
            .await
            .unwrap_err();
        assert!(matches!(err, CryptoError::Decrypt), "{what}: {err:?}");
    }
    let mut version = s.ciphertext.clone();
    version[0] = 2;
    assert!(matches!(
        f.keyring.decrypt(f.a, &aad, s.key_id, &version).await,
        Err(CryptoError::Malformed)
    ));
    assert!(matches!(
        f.keyring
            .decrypt(f.a, &aad, s.key_id, &s.ciphertext[..20])
            .await,
        Err(CryptoError::Malformed)
    ));
    f.db.cleanup().await;
}

#[tokio::test]
async fn wrong_key_id_fails() {
    let Some(f) = setup().await else { return };
    let aad = Aad::new("clinical_notes", "body_ct", f.a, Uuid::new_v4());
    let s = f.keyring.encrypt(f.a, &aad, NOTE).await.unwrap();
    let err = f
        .keyring
        .decrypt(f.a, &aad, Uuid::new_v4(), &s.ciphertext)
        .await
        .unwrap_err();
    assert!(matches!(err, CryptoError::KeyNotFound), "{err:?}");

    // Org B's real key id, used for org A's ciphertext: not org A's key.
    let other = f
        .keyring
        .encrypt(
            f.b,
            &Aad::new("clinical_notes", "body_ct", f.b, Uuid::new_v4()),
            b"x",
        )
        .await
        .unwrap();
    assert_ne!(other.key_id, s.key_id, "one DEK per org");
    let err = f
        .keyring
        .decrypt(f.a, &aad, other.key_id, &s.ciphertext)
        .await
        .unwrap_err();
    assert!(matches!(err, CryptoError::KeyNotFound), "{err:?}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn dek_is_wrapped_persisted_and_bound_to_the_kek() {
    let Some(f) = setup().await else { return };
    let aad = Aad::new("clinical_notes", "body_ct", f.a, Uuid::new_v4());
    let s = f.keyring.encrypt(f.a, &aad, NOTE).await.unwrap();

    let (wrapped, status): (Vec<u8>, String) = sqlx::query_as(
        "SELECT wrapped_dek, status FROM org_data_keys WHERE org_id = $1 AND key_id = $2",
    )
    .bind(f.a)
    .bind(s.key_id)
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    assert_eq!(status, "active");
    assert_eq!(wrapped[0], 1, "same envelope layout as the data");
    assert_eq!(wrapped.len(), 1 + 12 + 32 + 16, "a wrapped 256-bit key");

    // A fresh keyring with the same KEK (no cache) decrypts: the DEK lives in the database.
    let fresh = Keyring::new(Kek::from_base64(&f.kek_b64).unwrap(), f.db.pool.clone());
    assert_eq!(
        fresh
            .decrypt(f.a, &aad, s.key_id, &s.ciphertext)
            .await
            .unwrap(),
        NOTE
    );
    // Another KEK cannot unwrap it.
    let stranger = Keyring::new(
        Kek::from_base64(&random_kek_b64()).unwrap(),
        f.db.pool.clone(),
    );
    let err = stranger
        .decrypt(f.a, &aad, s.key_id, &s.ciphertext)
        .await
        .unwrap_err();
    assert!(matches!(err, CryptoError::KeyUnwrap), "{err:?}");

    // KEY-5 crypto-shredding: deleting the org's key rows makes its content unrecoverable.
    sqlx::query("DELETE FROM org_data_keys WHERE org_id = $1")
        .bind(f.a)
        .execute(&f.db.pool)
        .await
        .unwrap();
    let after = Keyring::new(Kek::from_base64(&f.kek_b64).unwrap(), f.db.pool.clone());
    assert!(matches!(
        after.decrypt(f.a, &aad, s.key_id, &s.ciphertext).await,
        Err(CryptoError::KeyNotFound)
    ));
    f.db.cleanup().await;
}

#[tokio::test]
async fn concurrent_first_use_creates_one_active_key() {
    let Some(f) = setup().await else { return };
    let kek = f.kek_b64.clone();
    let mut tasks = Vec::new();
    for _ in 0..12 {
        // Separate keyrings: no shared cache, so the database decides.
        let k = Keyring::new(Kek::from_base64(&kek).unwrap(), f.db.pool.clone());
        let org = f.a;
        tasks.push(tokio::spawn(async move {
            let aad = Aad::new("clinical_notes", "body_ct", org, Uuid::new_v4());
            k.encrypt(org, &aad, b"x").await.unwrap().key_id
        }));
    }
    let mut ids = Vec::new();
    for t in tasks {
        ids.push(t.await.unwrap());
    }
    ids.dedup();
    assert_eq!(ids.len(), 1, "{ids:?}");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM org_data_keys WHERE org_id = $1")
        .bind(f.a)
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
    f.db.cleanup().await;
}

// ---- blind index (KEY-4) ----------------------------------------------------------------

#[test]
fn normalisation_table() {
    for (input, want) in [
        (" 12-34 ", "1234"),
        ("1234", "1234"),
        ("ab 12", "AB12"),
        ("ab-12-c", "AB12C"),
        ("\t12 34\n", "1234"),
        // NFKC: full-width digits and letters fold to ASCII.
        ("１２３４", "1234"),
        ("ＭＲＮ-７", "MRN7"),
        // Unicode hyphen and non-breaking space.
        ("12\u{2010}34", "1234"),
        ("12\u{00A0}34", "1234"),
        ("tuth", "TUTH"),
    ] {
        assert_eq!(normalize(input), want, "{input:?}");
    }
}

#[tokio::test]
async fn blind_index_is_per_org_and_normalised() {
    let Some(f) = setup().await else { return };
    let a1 = f
        .keyring
        .mrn_blind_index(f.a, "TUTH", " 12-34 ")
        .await
        .unwrap();
    let a2 = f
        .keyring
        .mrn_blind_index(f.a, "tuth", "1234")
        .await
        .unwrap();
    assert_eq!(a1, a2, "normalisation variants match");
    assert_eq!(a1.len(), 32);
    let b = f
        .keyring
        .mrn_blind_index(f.b, "TUTH", "1234")
        .await
        .unwrap();
    assert_ne!(
        a1, b,
        "different orgs give different indexes for the same MRN"
    );
    let other_auth = f
        .keyring
        .mrn_blind_index(f.a, "AIIMS-DEL", "1234")
        .await
        .unwrap();
    assert_ne!(
        a1, other_auth,
        "the assigning authority is part of the index"
    );
    // The 0x1F separator keeps (authority, mrn) pairs apart.
    let x = f.keyring.mrn_blind_index(f.a, "AB", "C1").await.unwrap();
    let y = f.keyring.mrn_blind_index(f.a, "A", "BC1").await.unwrap();
    assert_ne!(x, y);
    // Stable across keyring instances (derived from the stored DEK).
    let fresh = Keyring::new(Kek::from_base64(&f.kek_b64).unwrap(), f.db.pool.clone());
    assert_eq!(
        fresh.mrn_blind_index(f.a, "TUTH", "12 34").await.unwrap(),
        a1
    );
    f.db.cleanup().await;
}

// ---- configuration and the 503 extractor (KEY-2) ----------------------------------------

fn config_with(kek: Option<&str>) -> Result<Config, ConfigError> {
    let jwt = common::test_jwt_key().to_owned();
    let kek = kek.map(str::to_owned);
    Config::from_lookup(move |key| match key {
        "DATABASE_URL" => Some(common::UNREACHABLE_DB_URL.to_owned()),
        "ASD_BLOB_BACKEND" => Some("memory".to_owned()),
        "ASD_JWT_SIGNING_KEY" => Some(jwt.clone()),
        "ASD_DATA_KEK" => kek.clone(),
        _ => None,
    })
}

#[test]
fn kek_config_is_optional_validated_and_never_printed() {
    let none = config_with(None).unwrap();
    assert!(none.data_kek.is_none());
    let b64 = random_kek_b64();
    let some = config_with(Some(&b64)).unwrap();
    assert!(some.data_kek.is_some());
    let dbg = format!("{some:?}");
    assert!(!dbg.contains(&b64), "{dbg}");
    for bad in [
        "not base64!",
        "c2hvcnQ=",
        &base64::engine::general_purpose::STANDARD.encode([7u8; 31]),
    ] {
        match config_with(Some(bad)).unwrap_err() {
            ConfigError::Invalid { name, reason } => {
                assert_eq!(name, "ASD_DATA_KEK");
                assert!(!reason.contains(bad), "{reason}");
            }
            other => panic!("{other:?}"),
        }
    }
}

async fn guarded(_crypto: ClinicalCrypto) -> &'static str {
    "ok"
}

fn dummy_router(state: AppState) -> Router {
    Router::new()
        .route("/clinical-dummy", get(guarded))
        .with_state(state)
}

#[tokio::test]
async fn missing_kek_means_no_keyring_and_503() {
    let config = config_with(None).unwrap();
    let pool = db::connect_lazy(&config).unwrap();
    let state = AppState::new(pool, Arc::new(MemoryBlobStore::new()), config);
    assert!(state.keyring.is_none(), "no KEK, no keyring");
    let (status, headers, body) = send(dummy_router(state), common::get("/clinical-dummy")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(common::content_type(&headers), "application/problem+json");
    assert_eq!(
        body["type"],
        "https://schemas.curv.org/asd/problems/clinical-crypto-unavailable"
    );
}

#[tokio::test]
async fn configured_kek_builds_a_keyring_and_the_extractor_passes() {
    let config = config_with(Some(&random_kek_b64())).unwrap();
    let pool = db::connect_lazy(&config).unwrap();
    let state = AppState::new(pool, Arc::new(MemoryBlobStore::new()), config);
    assert!(state.keyring.is_some());
    let router = Router::new()
        .route(
            "/clinical-dummy",
            get(|_c: ClinicalCrypto| async { StatusCode::NO_CONTENT }),
        )
        .with_state(state);
    let (status, _, _) = send(router, common::get("/clinical-dummy")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

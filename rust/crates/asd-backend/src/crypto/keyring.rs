//! owner: WP-C.9
//!
//! KEY-2, KEY-5: the key-encryption key ([`Kek`], `ASD_DATA_KEK`) and the per-org data keys
//! ([`Keyring`]).
//!
//! - Each org has one `active` 256-bit DEK in `org_data_keys`, created on first use, stored
//!   wrapped (AES-256-GCM under the KEK, AAD `"org_data_keys:wrapped_dek:{org}:{key_id}"`).
//! - Data keys are created and read through the keyring's own pool connection, never the
//!   caller's transaction: a key cached here must exist in the database even if the
//!   caller's transaction rolls back, or data encrypted with it later would be lost.
//! - Unwrapped keys are cached per process. Crypto-shredding an org (deleting its
//!   `org_data_keys` rows, KEY-5) therefore takes full effect once the API and workers
//!   restart; the runbook says so.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use aws_lc_rs::hmac;
use base64::Engine;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::aead::{random_key, AeadKey};
use super::{blind_index, Aad, CryptoError, Sealed};

/// The key-encryption key: 32 bytes from `ASD_DATA_KEK` (standard base64). `Debug` prints
/// only its fingerprint.
#[derive(Clone)]
pub struct Kek {
    bytes: Zeroizing<[u8; 32]>,
    id: String,
}

impl Kek {
    /// Parses standard base64 (with or without padding) of exactly 32 bytes. The error
    /// never repeats the input.
    pub fn from_base64(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let decoded = Zeroizing::new(
            base64::engine::general_purpose::STANDARD
                .decode(raw)
                .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(raw))
                .map_err(|_| "must be standard base64 of 32 random bytes".to_owned())?,
        );
        let bytes: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| format!("must decode to exactly 32 bytes (got {})", decoded.len()))?;
        Ok(Self::from_bytes(bytes))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let digest = Sha256::digest(bytes);
        let id: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        Self {
            bytes: Zeroizing::new(bytes),
            id: format!("kek-{id}"),
        }
    }

    /// Fingerprint stored in `org_data_keys.kek_id` (first 8 bytes of SHA-256, hex).
    pub fn id(&self) -> &str {
        &self.id
    }

    fn aead(&self) -> Result<AeadKey, CryptoError> {
        AeadKey::new(&self.bytes)
    }
}

impl PartialEq for Kek {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl fmt::Debug for Kek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kek").field("id", &self.id).finish()
    }
}

/// One unwrapped org data key.
struct DataKey {
    org: Uuid,
    key_id: Uuid,
    aead: AeadKey,
    raw: Zeroizing<[u8; 32]>,
}

fn wrap_aad(org: Uuid, key_id: Uuid) -> Aad {
    Aad::new("org_data_keys", "wrapped_dek", org, key_id)
}

/// Per-org data keys under one KEK. Cheap to share (`Arc<Keyring>` in `AppState`).
pub struct Keyring {
    kek: Kek,
    db: PgPool,
    keys: RwLock<HashMap<Uuid, Arc<DataKey>>>,
    active: RwLock<HashMap<Uuid, Uuid>>,
    index: RwLock<HashMap<Uuid, Arc<hmac::Key>>>,
}

impl fmt::Debug for Keyring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keyring").field("kek", &self.kek).finish()
    }
}

impl Keyring {
    pub fn new(kek: Kek, db: PgPool) -> Self {
        Self {
            kek,
            db,
            keys: RwLock::new(HashMap::new()),
            active: RwLock::new(HashMap::new()),
            index: RwLock::new(HashMap::new()),
        }
    }

    pub fn kek_id(&self) -> &str {
        self.kek.id()
    }

    /// Encrypts `plaintext` for `org` with its active data key (created on first use).
    pub async fn encrypt(
        &self,
        org: Uuid,
        aad: &Aad,
        plaintext: &[u8],
    ) -> Result<Sealed, CryptoError> {
        let key = self.active_key(org).await?;
        let ciphertext = key.aead.seal(aad.as_bytes(), plaintext)?;
        Ok(Sealed {
            ciphertext,
            key_id: key.key_id,
        })
    }

    /// [`encrypt`](Self::encrypt) for text.
    pub async fn encrypt_str(
        &self,
        org: Uuid,
        aad: &Aad,
        plaintext: &str,
    ) -> Result<Sealed, CryptoError> {
        self.encrypt(org, aad, plaintext.as_bytes()).await
    }

    /// Decrypts a ciphertext stored with `key_id`. The key must belong to `org`.
    pub async fn decrypt(
        &self,
        org: Uuid,
        aad: &Aad,
        key_id: Uuid,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let key = self.key(org, key_id).await?;
        key.aead.open(aad.as_bytes(), ciphertext)
    }

    /// [`decrypt`](Self::decrypt) for text written by [`encrypt_str`](Self::encrypt_str).
    pub async fn decrypt_string(
        &self,
        org: Uuid,
        aad: &Aad,
        key_id: Uuid,
        ciphertext: &[u8],
    ) -> Result<String, CryptoError> {
        let bytes = self.decrypt(org, aad, key_id, ciphertext).await?;
        String::from_utf8(bytes).map_err(|_| CryptoError::NotUtf8)
    }

    /// KEY-4 blind index of `(assigning_authority, mrn)` for `org`. The index key derives
    /// from the org's first data key, so it survives DEK rotation.
    pub async fn mrn_blind_index(
        &self,
        org: Uuid,
        assigning_authority: &str,
        mrn: &str,
    ) -> Result<[u8; 32], CryptoError> {
        let key = self.index_key(org).await?;
        Ok(blind_index::compute(&key, assigning_authority, mrn))
    }

    // ---- key loading ------------------------------------------------------------------

    fn cached(&self, key_id: Uuid) -> Option<Arc<DataKey>> {
        self.keys
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key_id)
            .cloned()
    }

    fn remember(&self, key: Arc<DataKey>) {
        self.keys
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.key_id, key);
    }

    fn unwrap_key(
        &self,
        org: Uuid,
        key_id: Uuid,
        wrapped: &[u8],
        kek_id: &str,
    ) -> Result<Arc<DataKey>, CryptoError> {
        let raw = self
            .kek
            .aead()?
            .open(wrap_aad(org, key_id).as_bytes(), wrapped)
            .map_err(|_| {
                tracing::error!(
                    %org,
                    %key_id,
                    stored_kek = kek_id,
                    configured_kek = self.kek.id(),
                    "org data key cannot be unwrapped with the configured ASD_DATA_KEK"
                );
                CryptoError::KeyUnwrap
            })?;
        let raw = Zeroizing::new(raw);
        let bytes: [u8; 32] = raw
            .as_slice()
            .try_into()
            .map_err(|_| CryptoError::KeyUnwrap)?;
        let key = Arc::new(DataKey {
            org,
            key_id,
            aead: AeadKey::new(&bytes)?,
            raw: Zeroizing::new(bytes),
        });
        self.remember(key.clone());
        Ok(key)
    }

    async fn key(&self, org: Uuid, key_id: Uuid) -> Result<Arc<DataKey>, CryptoError> {
        if let Some(k) = self.cached(key_id) {
            return if k.org == org {
                Ok(k)
            } else {
                Err(CryptoError::KeyNotFound)
            };
        }
        let row = sqlx::query!(
            "SELECT wrapped_dek, kek_id FROM org_data_keys WHERE key_id = $1 AND org_id = $2",
            key_id,
            org
        )
        .fetch_optional(&self.db)
        .await?
        .ok_or(CryptoError::KeyNotFound)?;
        self.unwrap_key(org, key_id, &row.wrapped_dek, &row.kek_id)
    }

    async fn active_key(&self, org: Uuid) -> Result<Arc<DataKey>, CryptoError> {
        let known = self
            .active
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&org)
            .copied();
        if let Some(k) = known.and_then(|id| self.cached(id)) {
            return Ok(k);
        }
        let row = match self.load_active(org).await? {
            Some(row) => row,
            None => {
                self.create_active(org).await?;
                self.load_active(org)
                    .await?
                    .ok_or_else(|| CryptoError::Internal("active data key vanished".into()))?
            }
        };
        let key = self.unwrap_key(org, row.0, &row.1, &row.2)?;
        self.active
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(org, key.key_id);
        Ok(key)
    }

    async fn load_active(&self, org: Uuid) -> Result<Option<(Uuid, Vec<u8>, String)>, CryptoError> {
        Ok(sqlx::query!(
            "SELECT key_id, wrapped_dek, kek_id FROM org_data_keys
             WHERE org_id = $1 AND status = 'active'",
            org
        )
        .fetch_optional(&self.db)
        .await?
        .map(|r| (r.key_id, r.wrapped_dek, r.kek_id)))
    }

    /// Inserts a new active key unless another process won the race (the partial unique
    /// index allows one active key per org). Autocommit on the keyring's pool.
    async fn create_active(&self, org: Uuid) -> Result<(), CryptoError> {
        let key_id = Uuid::new_v4();
        let dek = Zeroizing::new(random_key()?);
        let wrapped = self
            .kek
            .aead()?
            .seal(wrap_aad(org, key_id).as_bytes(), dek.as_slice())?;
        let inserted = sqlx::query!(
            "INSERT INTO org_data_keys (key_id, org_id, wrapped_dek, kek_id)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (org_id) WHERE status = 'active' DO NOTHING",
            key_id,
            org,
            wrapped,
            self.kek.id()
        )
        .execute(&self.db)
        .await?
        .rows_affected();
        if inserted == 1 {
            tracing::info!(%org, %key_id, kek_id = self.kek.id(), "created org data key");
        }
        Ok(())
    }

    async fn index_key(&self, org: Uuid) -> Result<Arc<hmac::Key>, CryptoError> {
        if let Some(k) = self
            .index
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&org)
            .cloned()
        {
            return Ok(k);
        }
        // Make sure the org has a key, then use its first one.
        self.active_key(org).await?;
        let first = sqlx::query!(
            "SELECT key_id FROM org_data_keys WHERE org_id = $1
             ORDER BY created_at, key_id LIMIT 1",
            org
        )
        .fetch_one(&self.db)
        .await?
        .key_id;
        let dek = self.key(org, first).await?;
        let key = Arc::new(blind_index::index_key(&dek.raw));
        self.index
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(org, key.clone());
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kek_parsing_and_debug() {
        let b64 = base64::engine::general_purpose::STANDARD.encode([9u8; 32]);
        let kek = Kek::from_base64(&b64).unwrap();
        assert!(kek.id().starts_with("kek-") && kek.id().len() == 4 + 16);
        assert_eq!(kek, Kek::from_base64(b64.trim_end_matches('=')).unwrap());
        assert!(!format!("{kek:?}").contains(&b64));
        assert!(Kek::from_base64("!!").is_err());
        let short = base64::engine::general_purpose::STANDARD.encode([9u8; 16]);
        assert!(Kek::from_base64(&short).unwrap_err().contains("32 bytes"));
        assert_ne!(kek, Kek::from_bytes([8u8; 32]));
    }
}

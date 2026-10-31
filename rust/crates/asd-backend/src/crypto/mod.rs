//! owner: WP-C.9
//!
//! Application-level envelope encryption for clinical content (KEY-1..7, ADR-0008 D5).
//! This module is the only code that calls the AEAD (KEY-6); handlers use [`Keyring`]:
//!
//! ```ignore
//! let id = Uuid::new_v4(); // the row id is part of the AAD: generate it first (trap 9)
//! let aad = Aad::new("clinical_notes", "body_ct", org, id);
//! let sealed = keyring.encrypt_str(org, &aad, &body).await?;
//! // INSERT ... (id, body_ct, body_key_id) VALUES ($1, $2, $3) with sealed.ciphertext/key_id
//! let body = keyring.decrypt_string(org, &aad, row.body_key_id, &row.body_ct).await?;
//! ```
//!
//! - AES-256-GCM, layout `version(1) || nonce(12) || ciphertext || tag(16)` ([`aead`]).
//! - AAD `"{table}:{column}:{org_id}:{row_id}"`, `column` = the `_ct` column name, so a
//!   ciphertext copied to another row, column or org fails to decrypt ([`Aad`]).
//! - One data key per org, wrapped by `ASD_DATA_KEK` ([`keyring`]).
//! - MRN blind index ([`blind_index`], [`Keyring::mrn_blind_index`]).
//! - Without `ASD_DATA_KEK`, `AppState.keyring` is `None` and the [`ClinicalCrypto`]
//!   extractor answers `503 clinical-crypto-unavailable`; the rest of the service runs.
//!
//! Plaintext is never logged: the redaction layer drops the clinical field names
//! (`telemetry::is_sensitive_field`), and nothing here formats plaintext into errors.

pub mod aead;
pub mod blind_index;
pub mod keyring;

use std::fmt;
use std::sync::Arc;

use asd_contract::problem::CLINICAL_CRYPTO_UNAVAILABLE;
use axum::extract::FromRequestParts;
use http::request::Parts;
use uuid::Uuid;

pub use keyring::{Kek, Keyring};

use crate::app::AppState;
use crate::error::AppError;

/// Additional authenticated data binding a ciphertext to its place (KEY-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aad(String);

impl Aad {
    /// `"{table}:{column}:{org_id}:{row_id}"`.
    pub fn new(table: &str, column: &str, org: Uuid, row: impl fmt::Display) -> Self {
        Self(format!("{table}:{column}:{org}:{row}"))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Display for Aad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A ciphertext and the data key that sealed it (`<col>_ct`, `<col>_key_id`).
#[derive(Clone, PartialEq, Eq)]
pub struct Sealed {
    pub ciphertext: Vec<u8>,
    pub key_id: Uuid,
}

impl fmt::Debug for Sealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sealed")
            .field("len", &self.ciphertext.len())
            .field("key_id", &self.key_id)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// `ASD_DATA_KEK` is not configured.
    #[error("clinical crypto unavailable: ASD_DATA_KEK is not configured")]
    Unavailable,
    /// No data key with this id for this org (wrong id, another org's key, or shredded).
    #[error("data key not found for this org")]
    KeyNotFound,
    /// The stored data key does not open with the configured KEK (wrong or rotated KEK).
    #[error("data key cannot be unwrapped with the configured KEK")]
    KeyUnwrap,
    /// Not a ciphertext of a known layout version.
    #[error("ciphertext malformed")]
    Malformed,
    /// Authentication failed: tampered bytes, wrong AAD or wrong key.
    #[error("decryption failed")]
    Decrypt,
    #[error("encryption failed")]
    Encrypt,
    #[error("decrypted text is not UTF-8")]
    NotUtf8,
    #[error("keyring: {0}")]
    Internal(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl From<CryptoError> for AppError {
    fn from(e: CryptoError) -> Self {
        match e {
            // A missing or wrong KEK: clinical content is unavailable, the rest works.
            CryptoError::Unavailable | CryptoError::KeyUnwrap => AppError::ServiceUnavailable {
                slug: CLINICAL_CRYPTO_UNAVAILABLE,
                detail: "clinical content encryption is not available on this server".into(),
            },
            CryptoError::Database(e) => AppError::Database(e),
            other => AppError::Internal(other.to_string()),
        }
    }
}

/// The keyring, or `503 clinical-crypto-unavailable`. Put it first in every handler that
/// reads or writes clinical content (KEY-2).
#[derive(Debug, Clone)]
pub struct ClinicalCrypto(pub Arc<Keyring>);

impl ClinicalCrypto {
    /// The same check for code that is not an extractor (e.g. a conditional write).
    pub fn from_state(state: &AppState) -> Result<Self, AppError> {
        state
            .keyring
            .clone()
            .map(Self)
            .ok_or_else(|| CryptoError::Unavailable.into())
    }
}

impl std::ops::Deref for ClinicalCrypto {
    type Target = Keyring;

    fn deref(&self) -> &Keyring {
        &self.0
    }
}

impl FromRequestParts<AppState> for ClinicalCrypto {
    type Rejection = AppError;

    async fn from_request_parts(_: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        Self::from_state(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aad_shape() {
        let org = Uuid::nil();
        assert_eq!(
            Aad::new("patient_refs", "mrn_ct", org, "row-1").to_string(),
            format!("patient_refs:mrn_ct:{org}:row-1")
        );
    }

    #[test]
    fn errors_map_to_problems_without_detail_leaks() {
        let p = AppError::from(CryptoError::Unavailable).to_problem();
        assert_eq!(p.status, 503);
        assert!(p.type_uri.ends_with("/clinical-crypto-unavailable"));
        let p = AppError::from(CryptoError::KeyUnwrap).to_problem();
        assert_eq!(p.status, 503);
        let p = AppError::from(CryptoError::Decrypt).to_problem();
        assert_eq!(p.status, 500);
        assert!(p.detail.is_none());
    }

    #[test]
    fn sealed_debug_hides_bytes() {
        let s = Sealed {
            ciphertext: vec![0xAB; 40],
            key_id: Uuid::nil(),
        };
        assert!(!format!("{s:?}").contains("171"));
    }
}

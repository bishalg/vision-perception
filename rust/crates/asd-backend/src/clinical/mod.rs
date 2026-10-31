//! owner: WP-C.10
//!
//! Clinician workspace logic behind the worklist and clinical review routes
//! (`docs/spec/modules/asd-clinician-and-interop/technical.md` CW-1/2, SIGN-1..10; ADR-0008):
//!
//! - [`review`]: the SIGN-1 state machine, the signed-content snapshot (SIGN-4, hashed with
//!   `asd_contract::clinical::content_hash`, RFC 8785 JCS), claim and draft save (SIGN-2/3)
//!   and the `ClinicalReview` read model (RBAC-4: content only with `ClinicalContentRead`).
//! - [`sign`]: sign, cosign and amend (SIGN-4..8, SIGN-10).
//! - [`reauth`]: step-up re-authentication, password and TOTP (SIGN-5).
//!
//! Concurrency rule used throughout: every change to a review's content or state bumps
//! `clinical_reviews.row_version`. Work that needs the keyring or Argon2 (decrypt, encrypt,
//! hashing, password verify) runs **before** the transaction against the version read
//! then; the transaction locks the review row (`FOR UPDATE`) and proceeds only if
//! `row_version` is still the one read, so a concurrent change is refused instead of
//! overwritten, and no pool connection is requested while a row lock is held.
//!
//! The same types live in `asd-export::model` (`ReviewState`, `Disposition`) for exports;
//! `asd-contract::clinical` is the source of truth and the spellings match.

pub mod reauth;
pub mod review;
pub mod sign;

use asd_contract::clinical::{settings as keys, CosignPolicy, WorklistScope};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// The clinical keys of `organizations.settings` this package reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClinicalSettings {
    pub cosign_policy: CosignPolicy,
    pub worklist_scope: WorklistScope,
}

impl ClinicalSettings {
    /// CW-2 / SIGN-6 defaults: scope `all` for `clinic` orgs and `flagged` otherwise;
    /// cosign `none`. A stored value that does not parse (the PATCH route validates, so
    /// only a hand-edited row can have one) fails safe: cosign `always`, scope `all`.
    pub fn from_settings(settings: &Value, org_kind: &str) -> Self {
        let clinical = settings.get(keys::CLINICAL);
        let get = |key: &str| clinical.and_then(|c| c.get(key));
        let cosign_policy = match get(keys::COSIGN_POLICY) {
            None | Some(Value::Null) => CosignPolicy::None,
            Some(v) => v.as_str().and_then(CosignPolicy::parse).unwrap_or_else(|| {
                tracing::warn!(
                    "organizations.settings.clinical.cosign_policy is invalid; using always"
                );
                CosignPolicy::Always
            }),
        };
        let default_scope = if org_kind == "clinic" {
            WorklistScope::All
        } else {
            WorklistScope::Flagged
        };
        let worklist_scope = match get(keys::WORKLIST_SCOPE) {
            None | Some(Value::Null) => default_scope,
            Some(v) => v
                .as_str()
                .and_then(WorklistScope::parse)
                .unwrap_or(WorklistScope::All),
        };
        Self {
            cosign_policy,
            worklist_scope,
        }
    }

    /// Reads the org's settings.
    pub async fn load(conn: &mut PgConnection, org: Uuid) -> AppResult<Self> {
        let row = sqlx::query!(
            "SELECT kind, settings FROM organizations WHERE id = $1",
            org
        )
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(AppError::NotFound)?;
        Ok(Self::from_settings(&row.settings, &row.kind))
    }
}

/// SIGN-6: whether a session with `attention_flag` needs a second signature.
pub fn cosign_required(policy: CosignPolicy, attention_flag: &str) -> bool {
    match policy {
        CosignPolicy::None => false,
        CosignPolicy::Always => true,
        CosignPolicy::RedFlag => attention_flag == "red",
    }
}

/// CW-2: flags kept by `worklist_scope = flagged`.
pub const FLAGGED: &[&str] = &["red", "yellow", "unreliable"];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_defaults_and_fail_safe() {
        let s = ClinicalSettings::from_settings(&json!({}), "clinic");
        assert_eq!(s.cosign_policy, CosignPolicy::None);
        assert_eq!(s.worklist_scope, WorklistScope::All);
        let s = ClinicalSettings::from_settings(&json!({}), "research");
        assert_eq!(s.worklist_scope, WorklistScope::Flagged);
        let s = ClinicalSettings::from_settings(
            &json!({"clinical": {"cosign_policy": "red_flag", "worklist_scope": "flagged"}}),
            "clinic",
        );
        assert_eq!(s.cosign_policy, CosignPolicy::RedFlag);
        assert_eq!(s.worklist_scope, WorklistScope::Flagged);
        let s = ClinicalSettings::from_settings(
            &json!({"clinical": {"cosign_policy": "sometimes", "worklist_scope": 3}}),
            "home",
        );
        assert_eq!(s.cosign_policy, CosignPolicy::Always);
        assert_eq!(s.worklist_scope, WorklistScope::All);
    }

    #[test]
    fn cosign_policy_table() {
        for flag in ["green", "yellow", "red", "unreliable"] {
            assert!(!cosign_required(CosignPolicy::None, flag));
            assert!(cosign_required(CosignPolicy::Always, flag));
            assert_eq!(cosign_required(CosignPolicy::RedFlag, flag), flag == "red");
        }
    }
}

//! owner: WP-B.4
//!
//! Config documents and signed bundles (CFG-1, CFG-2, CON-6 server side, 02 ADR-B6).
//!
//! - [`lifecycle`]: documents of kind `stimulus_catalog | normative_baselines |
//!   scoring_profile` move `draft -> candidate (approve) -> published -> retired`, with the
//!   four-eyes rule: nobody approves or publishes a document they authored, and a
//!   system-seeded document is approved and published by two different people.
//! - [`bundle`]: composes the three published documents into a `ConfigBundle`, signs it
//!   with `ASD_BUNDLE_SIGNING_KEY` ([`bundle::BundleSigner`], never the JWT key), stores it
//!   exactly as served and delivers it to devices with ETag / If-None-Match.
//! - [`seed`]: on first boot with no documents, drafts from the kit's bundled JSON plus the
//!   default `ScoringProfile`; never published automatically.
//!
//! Documents are global (the schema has no `org_id` on `config_documents`); bundles are
//! global (`org_id NULL`) or per org.

use asd_contract::api_v1::{ConfigDocumentKind, ConfigDocumentStatus};
use axum::response::{IntoResponse, Response};
use http::StatusCode;

use crate::error::AppError;
use crate::problem::Problem;

pub mod bundle;
pub mod lifecycle;
pub mod seed;

pub const ALL_KINDS: [ConfigDocumentKind; 3] = [
    ConfigDocumentKind::StimulusCatalog,
    ConfigDocumentKind::NormativeBaselines,
    ConfigDocumentKind::ScoringProfile,
];

pub fn kind_str(kind: ConfigDocumentKind) -> &'static str {
    match kind {
        ConfigDocumentKind::StimulusCatalog => "stimulus_catalog",
        ConfigDocumentKind::NormativeBaselines => "normative_baselines",
        ConfigDocumentKind::ScoringProfile => "scoring_profile",
    }
}

pub fn kind_from_str(s: &str) -> Option<ConfigDocumentKind> {
    ALL_KINDS.into_iter().find(|k| kind_str(*k) == s)
}

pub fn status_str(status: ConfigDocumentStatus) -> &'static str {
    match status {
        ConfigDocumentStatus::Draft => "draft",
        ConfigDocumentStatus::Candidate => "candidate",
        ConfigDocumentStatus::Published => "published",
        ConfigDocumentStatus::Retired => "retired",
    }
}

pub fn status_from_str(s: &str) -> Option<ConfigDocumentStatus> {
    [
        ConfigDocumentStatus::Draft,
        ConfigDocumentStatus::Candidate,
        ConfigDocumentStatus::Published,
        ConfigDocumentStatus::Retired,
    ]
    .into_iter()
    .find(|st| status_str(*st) == s)
}

/// Errors of the config routes: every [`AppError`], plus the problem types that only
/// this module returns (their slugs are part of the API).
#[derive(Debug, thiserror::Error)]
pub enum ConfigApiError {
    #[error(transparent)]
    App(#[from] AppError),
    /// 403 `four-eyes-required`: the caller authored (or approved) this document.
    #[error("four-eyes rule: {0}")]
    FourEyes(&'static str),
    /// 503 `bundle-signing-unavailable`: `ASD_BUNDLE_SIGNING_KEY` is not configured.
    #[error("bundle signing key not configured")]
    SigningUnavailable,
}

impl From<sqlx::Error> for ConfigApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::App(AppError::Database(e))
    }
}

pub type ConfigApiResult<T> = Result<T, ConfigApiError>;

impl IntoResponse for ConfigApiError {
    fn into_response(self) -> Response {
        match self {
            Self::App(e) => e.into_response(),
            Self::FourEyes(detail) => Problem::new(StatusCode::FORBIDDEN, "four-eyes-required")
                .with_detail(detail)
                .into_response(),
            Self::SigningUnavailable => {
                tracing::error!("POST /v1/config/bundles without ASD_BUNDLE_SIGNING_KEY");
                Problem::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "bundle-signing-unavailable",
                )
                .with_detail("the server has no bundle signing key configured")
                .into_response()
            }
        }
    }
}

pub(crate) fn conflict(slug: &'static str, detail: impl Into<String>) -> ConfigApiError {
    ConfigApiError::App(AppError::Conflict {
        slug,
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_and_status_strings_round_trip_and_match_serde() {
        for kind in ALL_KINDS {
            assert_eq!(kind_from_str(kind_str(kind)), Some(kind));
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::Value::from(kind_str(kind))
            );
        }
        for s in ["draft", "candidate", "published", "retired"] {
            let st = status_from_str(s).unwrap();
            assert_eq!(status_str(st), s);
            assert_eq!(
                serde_json::to_value(st).unwrap(),
                serde_json::Value::from(s)
            );
        }
        assert_eq!(kind_from_str("feature_flags"), None);
        assert_eq!(status_from_str("live"), None);
    }

    #[test]
    fn module_problems_have_their_own_slugs() {
        let r = ConfigApiError::FourEyes("x").into_response();
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let r = ConfigApiError::SigningUnavailable.into_response();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        let r = ConfigApiError::App(AppError::NotFound).into_response();
        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }
}

//! owner: WP-B.1
//!
//! [`AppError`]: the error type handlers return. Each variant maps to one problem type;
//! internal causes are logged (without PHI, see `telemetry`) and never sent to clients.

use axum::response::{IntoResponse, Response};
use http::StatusCode;

use crate::blob::BlobError;
use crate::problem::{InvalidParam, Problem};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 400 `validation-failed` with `invalid_params` (CON-14).
    #[error("validation failed")]
    Validation(Vec<InvalidParam>),
    /// 400 with a specific slug (e.g. `unsupported-schema-version`, CON-21).
    #[error("bad request: {slug}")]
    BadRequest { slug: &'static str, detail: String },
    /// 401 `unauthorized`.
    #[error("unauthorized")]
    Unauthorized,
    /// 403 `forbidden`.
    #[error("forbidden")]
    Forbidden,
    /// 404 `not-found`; also used for other orgs' resources (AUTH-4).
    #[error("not found")]
    NotFound,
    /// 409 with a specific slug (e.g. `idempotency-key-reused`, CON-17).
    #[error("conflict: {slug}")]
    Conflict { slug: &'static str, detail: String },
    /// 410 with a specific slug (e.g. `enrol-code-exhausted`, CON-11).
    #[error("gone: {slug}")]
    Gone { slug: &'static str, detail: String },
    /// 413 `payload-too-large`.
    #[error("payload too large")]
    PayloadTooLarge,
    /// 429 `rate-limited` with `Retry-After` seconds.
    #[error("rate limited")]
    RateLimited { retry_after_secs: u64 },
    /// 503 `service-unavailable`.
    #[error("service unavailable")]
    Unavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Blob(#[from] BlobError),
    /// 500 `internal`; the message is logged, not returned.
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn to_problem(&self) -> Problem {
        match self {
            Self::Validation(params) => Problem::new(StatusCode::BAD_REQUEST, "validation-failed")
                .with_invalid_params(params.clone()),
            Self::BadRequest { slug, detail } => {
                Problem::new(StatusCode::BAD_REQUEST, slug).with_detail(detail.clone())
            }
            Self::Unauthorized => Problem::new(StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden => Problem::new(StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound => Problem::not_found(),
            Self::Conflict { slug, detail } => {
                Problem::new(StatusCode::CONFLICT, slug).with_detail(detail.clone())
            }
            Self::Gone { slug, detail } => {
                Problem::new(StatusCode::GONE, slug).with_detail(detail.clone())
            }
            Self::PayloadTooLarge | Self::Blob(BlobError::TooLarge { .. }) => {
                Problem::new(StatusCode::PAYLOAD_TOO_LARGE, "payload-too-large")
            }
            Self::RateLimited { retry_after_secs } => {
                Problem::new(StatusCode::TOO_MANY_REQUESTS, "rate-limited")
                    .with_extension("retry_after", (*retry_after_secs).into())
            }
            Self::Unavailable
            | Self::Blob(BlobError::Unavailable(_))
            | Self::Database(sqlx::Error::PoolTimedOut) => {
                Problem::new(StatusCode::SERVICE_UNAVAILABLE, "service-unavailable")
            }
            Self::Database(_) | Self::Blob(_) | Self::Internal(_) => {
                Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let problem = self.to_problem();
        if problem.status >= 500 {
            // Display of sqlx/blob errors carries no request data.
            tracing::error!(error = %self, "request failed");
        }
        let mut response = problem.into_response();
        if let Self::RateLimited { retry_after_secs } = self {
            response
                .headers_mut()
                .insert(http::header::RETRY_AFTER, retry_after_secs.into());
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(e: AppError) -> (u16, String) {
        let p = e.to_problem();
        (p.status, p.type_uri.rsplit('/').next().unwrap().to_owned())
    }

    #[test]
    fn variants_map_to_status_and_slug() {
        let s = |slug: &'static str| (slug, String::new());
        assert_eq!(
            status(AppError::Validation(vec![])),
            (400, "validation-failed".into())
        );
        let (slug, detail) = s("unsupported-schema-version");
        assert_eq!(
            status(AppError::BadRequest { slug, detail }),
            (400, slug.into())
        );
        assert_eq!(status(AppError::Unauthorized), (401, "unauthorized".into()));
        assert_eq!(status(AppError::Forbidden), (403, "forbidden".into()));
        assert_eq!(status(AppError::NotFound), (404, "not-found".into()));
        let (slug, detail) = s("idempotency-key-reused");
        assert_eq!(
            status(AppError::Conflict { slug, detail }),
            (409, slug.into())
        );
        let (slug, detail) = s("enrol-code-exhausted");
        assert_eq!(status(AppError::Gone { slug, detail }), (410, slug.into()));
        assert_eq!(
            status(AppError::PayloadTooLarge),
            (413, "payload-too-large".into())
        );
        assert_eq!(
            status(AppError::RateLimited {
                retry_after_secs: 5
            }),
            (429, "rate-limited".into())
        );
        assert_eq!(
            status(AppError::Unavailable),
            (503, "service-unavailable".into())
        );
        assert_eq!(
            status(AppError::Database(sqlx::Error::PoolTimedOut)),
            (503, "service-unavailable".into())
        );
        assert_eq!(
            status(AppError::Database(sqlx::Error::RowNotFound)),
            (500, "internal".into())
        );
        assert_eq!(
            status(AppError::Internal("x".into())),
            (500, "internal".into())
        );
    }

    #[test]
    fn internal_details_are_not_in_the_body() {
        let p = AppError::Internal("db password is hunter2".into()).to_problem();
        assert!(!serde_json::to_string(&p).unwrap().contains("hunter2"));
    }

    #[test]
    fn rate_limited_sets_retry_after() {
        let r = AppError::RateLimited {
            retry_after_secs: 17,
        }
        .into_response();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(r.headers()[http::header::RETRY_AFTER], "17");
    }
}

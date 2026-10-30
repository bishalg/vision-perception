//! owner: WP-B.1
//!
//! RFC 7807 `application/problem+json` responses. Every error the service returns goes
//! through [`Problem`]; `type` is `https://schemas.curv.org/asd/problems/<slug>`.
//!
//! The wire shape is mirrored by `asd-contract::problem` (WP-R.3) for schema export; when
//! that type lands, this struct can wrap it without changing the bytes.

use axum::response::{IntoResponse, Response};
use http::{header, HeaderValue, StatusCode};
use serde::Serialize;
use serde_json::{Map, Value};

pub const PROBLEM_TYPE_BASE: &str = "https://schemas.curv.org/asd/problems/";
pub const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// One invalid request parameter (`invalid_params[]`, CON-14).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InvalidParam {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub invalid_params: Vec<InvalidParam>,
    /// Extension members (RFC 7807 §3.2), flattened into the top-level object.
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl Problem {
    /// A problem with `type = PROBLEM_TYPE_BASE + slug` and the status's canonical title.
    pub fn new(status: StatusCode, slug: &str) -> Self {
        Self {
            type_uri: format!("{PROBLEM_TYPE_BASE}{slug}"),
            title: status.canonical_reason().unwrap_or("Error").to_owned(),
            status: status.as_u16(),
            detail: None,
            instance: None,
            invalid_params: Vec::new(),
            extensions: Map::new(),
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_invalid_params(mut self, params: Vec<InvalidParam>) -> Self {
        self.invalid_params = params;
        self
    }

    pub fn with_extension(mut self, key: &str, value: Value) -> Self {
        self.extensions.insert(key.to_owned(), value);
        self
    }

    pub fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not-found")
    }

    pub fn method_not_allowed() -> Self {
        Self::new(StatusCode::METHOD_NOT_ALLOWED, "method-not-allowed")
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = serde_json::to_vec(&self).unwrap_or_else(|_| b"{}".to_vec());
        let mut response = (status, body).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(PROBLEM_CONTENT_TYPE),
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_rfc7807_members_and_extensions() {
        let p = Problem::new(StatusCode::BAD_REQUEST, "validation-failed")
            .with_detail("sessionId is required")
            .with_invalid_params(vec![InvalidParam {
                name: "sessionId".into(),
                reason: "missing".into(),
            }])
            .with_extension("trace", Value::from("t-1"));
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "type": "https://schemas.curv.org/asd/problems/validation-failed",
                "title": "Bad Request",
                "status": 400,
                "detail": "sessionId is required",
                "invalid_params": [{"name": "sessionId", "reason": "missing"}],
                "trace": "t-1"
            })
        );
    }

    #[test]
    fn optional_members_are_omitted() {
        let v = serde_json::to_value(Problem::not_found()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "type": "https://schemas.curv.org/asd/problems/not-found",
                "title": "Not Found",
                "status": 404
            })
        );
    }

    #[test]
    fn response_has_problem_content_type_and_status() {
        let r = Problem::new(StatusCode::CONFLICT, "idempotency-key-reused").into_response();
        assert_eq!(r.status(), StatusCode::CONFLICT);
        assert_eq!(r.headers()[header::CONTENT_TYPE], PROBLEM_CONTENT_TYPE);
    }
}

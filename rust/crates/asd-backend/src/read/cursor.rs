//! owner: WP-B.3
//!
//! Opaque list cursors (API conventions): base64url (no padding) of
//! `{"t": "<RFC 3339 timestamp>", "id": "<id>"}`, the sort key of the last item returned.
//! Lists sort by `(timestamp DESC, id DESC)` and continue strictly after the cursor.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::problem::InvalidParam;

pub const DEFAULT_LIMIT: i64 = 50;
pub const MAX_LIMIT: i64 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    #[serde(rename = "t")]
    pub at: DateTime<Utc>,
    pub id: String,
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

impl Cursor {
    pub fn new(at: DateTime<Utc>, id: impl Into<String>) -> Self {
        Self { at, id: id.into() }
    }

    pub fn encode(&self) -> String {
        let json = serde_json::json!({
            "t": self.at.to_rfc3339_opts(SecondsFormat::Micros, true),
            "id": self.id,
        });
        URL_SAFE_NO_PAD.encode(json.to_string())
    }

    pub fn decode(raw: &str) -> Result<Self, AppError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(raw)
            .map_err(|_| invalid("cursor", "not a cursor from this API"))?;
        serde_json::from_slice(&bytes).map_err(|_| invalid("cursor", "not a cursor from this API"))
    }
}

/// `limit` in `1..=100`, default 50.
pub fn limit(raw: Option<i64>) -> Result<i64, AppError> {
    match raw {
        None => Ok(DEFAULT_LIMIT),
        Some(n) if (1..=MAX_LIMIT).contains(&n) => Ok(n),
        Some(_) => Err(invalid("limit", "must be between 1 and 100")),
    }
}

/// RFC 3339 UTC with milliseconds, as the device writes dates.
pub fn ts(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_keeps_microseconds() {
        let at = DateTime::parse_from_rfc3339("2026-10-05T10:00:00.123456Z")
            .unwrap()
            .with_timezone(&Utc);
        let c = Cursor::new(at, "S-5");
        let encoded = c.encode();
        assert!(encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_eq!(Cursor::decode(&encoded).unwrap(), c);
    }

    #[test]
    fn garbage_is_rejected() {
        for bad in ["", "not-a-cursor", "e30", "!!!"] {
            assert!(Cursor::decode(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn limits() {
        assert_eq!(limit(None).unwrap(), 50);
        assert_eq!(limit(Some(100)).unwrap(), 100);
        assert!(limit(Some(0)).is_err());
        assert!(limit(Some(101)).is_err());
    }
}

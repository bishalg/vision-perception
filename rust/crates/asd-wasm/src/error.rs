//! owner: WP-R.6
//!
//! The boundary's error and the JSON envelope every export returns.
//!
//! Success: `{"ok":true,"result":<value>}`. Failure:
//! `{"ok":false,"error":{"kind":"<kind>","message":"<text>"}}` with `kind` one of
//! `invalidJson`, `invalidArgument`, `unsupportedSchemaVersion`. A panic cannot be returned
//! (wasm32 aborts); the hook in `lib.rs` keeps its message for `lastPanic()`, which the
//! TypeScript wrapper turns into `{"ok":false,"error":{"kind":"panic",...}}`.

use serde::Serialize;
use serde_json::{json, Value};

/// Why a call failed. The variant is the `kind`; the text is for logs and the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    /// Not JSON, or JSON that does not match the expected document.
    InvalidJson(String),
    /// A scalar or option is unusable.
    InvalidArgument(String),
    /// A session whose `schemaVersion` this build does not read.
    UnsupportedSchemaVersion(String),
}

impl BoundaryError {
    pub fn invalid_json(what: &str, e: impl std::fmt::Display) -> Self {
        Self::InvalidJson(format!("{what}: {e}"))
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidJson(_) => "invalidJson",
            Self::InvalidArgument(_) => "invalidArgument",
            Self::UnsupportedSchemaVersion(_) => "unsupportedSchemaVersion",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::InvalidJson(m) | Self::InvalidArgument(m) | Self::UnsupportedSchemaVersion(m) => {
                m
            }
        }
    }
}

/// Wraps a result in the envelope. Serialising a result never fails for the types used
/// here (strings, finite or non-finite floats become numbers or `null`), but a failure is
/// still reported as an envelope rather than a panic.
pub fn envelope<T: Serialize>(result: Result<T, BoundaryError>) -> String {
    let value = match result {
        Ok(v) => match serde_json::to_value(&v) {
            Ok(result) => json!({"ok": true, "result": result}),
            Err(e) => error_value(&BoundaryError::invalid_json("result", e)),
        },
        Err(e) => error_value(&e),
    };
    value.to_string()
}

fn error_value(e: &BoundaryError) -> Value {
    json!({"ok": false, "error": {"kind": e.kind(), "message": e.message()}})
}

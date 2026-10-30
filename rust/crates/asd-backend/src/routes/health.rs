//! owner: WP-B.1
//!
//! CON-22: `GET /health` is liveness (process up, no dependencies touched);
//! `GET /health/ready` checks the database, that every embedded migration has been
//! applied, and the blob store. There is no `/v1/health`.

use std::time::Duration;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use http::StatusCode;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::db;
use crate::problem::Problem;

/// Upper bound for each readiness probe, so a hung dependency cannot hang the probe.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(live))
        .route("/health/ready", get(ready))
}

async fn live() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "asd-backend",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    Ok,
    Unavailable,
    Pending,
    Unknown,
}

impl Check {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unavailable => "unavailable",
            Self::Pending => "pending",
            Self::Unknown => "unknown",
        }
    }
}

async fn database_checks(state: &AppState) -> (Check, Check) {
    match tokio::time::timeout(PROBE_TIMEOUT, db::ping(&state.db)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "readiness: database unavailable");
            return (Check::Unavailable, Check::Unknown);
        }
        Err(_) => {
            tracing::warn!("readiness: database probe timed out");
            return (Check::Unavailable, Check::Unknown);
        }
    }
    let migrations =
        match tokio::time::timeout(PROBE_TIMEOUT, db::pending_migrations(&state.db)).await {
            Ok(Ok(pending)) if pending.is_empty() => Check::Ok,
            Ok(Ok(pending)) => {
                tracing::warn!(
                    ?pending,
                    "readiness: migrations not applied (run `asd-backend migrate`)"
                );
                Check::Pending
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "readiness: cannot read migration state");
                Check::Unknown
            }
            Err(_) => Check::Unknown,
        };
    (Check::Ok, migrations)
}

async fn blob_check(state: &AppState) -> Check {
    match tokio::time::timeout(PROBE_TIMEOUT, state.blobs.ready()).await {
        Ok(Ok(())) => Check::Ok,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "readiness: blob store unavailable");
            Check::Unavailable
        }
        Err(_) => {
            tracing::warn!("readiness: blob store probe timed out");
            Check::Unavailable
        }
    }
}

async fn ready(State(state): State<AppState>) -> Response {
    let ((database, migrations), blob_store) =
        tokio::join!(database_checks(&state), blob_check(&state));
    let checks = json!({
        "database": database.as_str(),
        "migrations": migrations.as_str(),
        "blob_store": blob_store.as_str(),
    });
    if [database, migrations, blob_store]
        .iter()
        .all(|c| *c == Check::Ok)
    {
        (
            StatusCode::OK,
            Json(json!({ "status": "ready", "checks": checks })),
        )
            .into_response()
    } else {
        Problem::new(StatusCode::SERVICE_UNAVAILABLE, "not-ready")
            .with_title("Service Not Ready")
            .with_detail("one or more dependencies are not ready")
            .with_extension("checks", checks)
            .into_response()
    }
}

//! owner: WP-B.1
//!
//! The Axum application: shared state, middleware stack and route tree. Route modules
//! (B.2, B.3, ...) register under [`crate::routes::v1`]; nothing else needs to change here.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::MatchedPath;
use axum::response::{IntoResponse, Response};
use axum::Router;
use http::{header, HeaderName, HeaderValue, Method, Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::blob::{BlobStore, MemoryBlobStore, S3BlobStore};
use crate::config::{BlobBackend, Config};
use crate::problem::Problem;
use crate::routes;

/// State shared by every handler. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub blobs: Arc<dyn BlobStore>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: PgPool, blobs: Arc<dyn BlobStore>, config: Config) -> Self {
        Self {
            db,
            blobs,
            config: Arc::new(config),
        }
    }
}

/// The blob store selected by `ASD_BLOB_BACKEND`.
pub async fn build_blob_store(config: &Config) -> Arc<dyn BlobStore> {
    match config.blob_backend {
        BlobBackend::S3 => Arc::new(S3BlobStore::from_env(&config.s3).await),
        BlobBackend::Memory => Arc::new(MemoryBlobStore::new()),
    }
}

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Builds the full router with its middleware:
/// request id (set + propagate), PHI-safe request tracing, timeout, optional CORS
/// restricted to the console origin, and problem+json for 404/405.
pub fn router(state: AppState) -> Router {
    let timeout = state.config.request_timeout;
    let cors = state.config.console_origin.as_deref().map(cors_layer);

    Router::new()
        .merge(routes::health::router())
        .merge(routes::v1())
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(REQUEST_ID, MakeRequestUuid))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(request_span)
                        .on_request(())
                        .on_response(log_response),
                )
                .layer(PropagateRequestIdLayer::new(REQUEST_ID))
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    timeout,
                ))
                .option_layer(cors),
        )
        .with_state(state)
}

/// The span records the route *template* (`/v1/participants/{token}/summary`), never the
/// raw path or query string, which may carry participant tokens (OBS-1).
fn request_span<B>(request: &Request<B>) -> Span {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("<unmatched>", MatchedPath::as_str);
    let request_id = request
        .headers()
        .get(&REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    tracing::info_span!("http", method = %request.method(), route, request_id)
}

fn log_response<B>(response: &http::Response<B>, latency: Duration, _span: &Span) {
    tracing::info!(
        status = response.status().as_u16(),
        latency_ms = u64::try_from(latency.as_millis()).unwrap_or(u64::MAX),
        "response"
    );
}

fn cors_layer(origin: &str) -> CorsLayer {
    let origin = HeaderValue::from_str(origin).expect("validated by Config");
    CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::IF_NONE_MATCH,
            REQUEST_ID,
            HeaderName::from_static("x-idempotency-key"),
            HeaderName::from_static("x-schema-version"),
        ])
        .expose_headers([header::ETAG, header::RETRY_AFTER, REQUEST_ID])
        .max_age(Duration::from_secs(600))
}

async fn not_found() -> Response {
    Problem::not_found().into_response()
}

async fn method_not_allowed() -> Response {
    Problem::method_not_allowed().into_response()
}

//! owner: WP-B.13
//!
//! Prometheus metrics (OBS-2, OBS-3). Not to be confused with `routes::metrics`, the
//! per-org dashboard under `/v1/orgs/{id}/metrics`.
//!
//! - One process-wide registry ([`render`]); the HTTP middleware ([`track`], installed by
//!   `app::router`) records `http_requests_total{route,method,status}` and
//!   `http_request_duration_seconds{route,method}`; the auth rate limiters call
//!   [`rate_limited`]; the exposition handler refreshes `jobs_rows{kind,status}` (one
//!   `GROUP BY`, cached [`JOBS_CACHE_TTL`]) and serves everything as text exposition 0.0.4.
//! - The exposition lives on its own listener ([`metrics_router`], started by
//!   `asd-backend serve` only when `ASD_METRICS_BIND_ADDR` is set). The API router never
//!   serves `/metrics`, and the scrape router has no tracing layer, so scrapes are not logged.
//! - Label safety: `route` is the matched path *template* (`/v1/sessions/{id}`) or the fixed
//!   [`UNMATCHED`]; `method` is a standard verb or `OTHER`; `status` is a three-digit code;
//!   `limiter`, `kind` and `status` of jobs come from closed sets. Nothing derived from a
//!   path parameter, header, body or user ever becomes a label value.

use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
};
use sqlx::PgPool;
use tokio::sync::Mutex;

/// `route` label of a request that matched no route (404 fallback).
pub const UNMATCHED: &str = "unmatched";

/// OBS-3 histogram buckets, seconds: 5 ms .. 10 s.
pub const BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// How long the `jobs_rows` gauge is reused between scrapes.
pub const JOBS_CACHE_TTL: Duration = Duration::from_secs(10);

/// Upper bound on the jobs query inside a scrape (Prometheus' default scrape timeout is 10 s).
const JOBS_QUERY_TIMEOUT: Duration = Duration::from_secs(2);

/// The `limiter` label values (OBS-3). Each is initialised to 0 so the series exist before
/// the first rejection.
pub const LIMITERS: [&str; 5] = [
    "login_ip",
    "login_user",
    "totp_user",
    "register_ip",
    "ingest_device",
];

const CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

struct Metrics {
    registry: Registry,
    requests: IntCounterVec,
    duration: HistogramVec,
    rate_limited: IntCounterVec,
    jobs_rows: IntGaugeVec,
}

static METRICS: LazyLock<Metrics> = LazyLock::new(|| {
    let registry = Registry::new();
    let requests = IntCounterVec::new(
        Opts::new("http_requests_total", "HTTP requests by route template."),
        &["route", "method", "status"],
    )
    .expect("valid metric");
    let duration = HistogramVec::new(
        HistogramOpts::new(
            "http_request_duration_seconds",
            "HTTP request latency by route template.",
        )
        .buckets(BUCKETS.to_vec()),
        &["route", "method"],
    )
    .expect("valid metric");
    let rate_limited = IntCounterVec::new(
        Opts::new(
            "auth_rate_limited_total",
            "Requests rejected by an auth rate limiter.",
        ),
        &["limiter"],
    )
    .expect("valid metric");
    for limiter in LIMITERS {
        rate_limited.with_label_values(&[limiter]);
    }
    let jobs_rows = IntGaugeVec::new(
        Opts::new("jobs_rows", "Rows in the jobs table by kind and status."),
        &["kind", "status"],
    )
    .expect("valid metric");
    let build_info = IntGaugeVec::new(
        Opts::new("asd_backend_build_info", "Build information; always 1."),
        &["version", "git_sha"],
    )
    .expect("valid metric");
    build_info
        .with_label_values(&[
            env!("CARGO_PKG_VERSION"),
            option_env!("ASD_GIT_SHA").unwrap_or("unknown"),
        ])
        .set(1);
    for c in [
        Box::new(requests.clone()) as Box<dyn prometheus::core::Collector>,
        Box::new(duration.clone()),
        Box::new(rate_limited.clone()),
        Box::new(jobs_rows.clone()),
        Box::new(build_info),
    ] {
        registry.register(c).expect("metrics register once");
    }
    Metrics {
        registry,
        requests,
        duration,
        rate_limited,
        jobs_rows,
    }
});

/// Counts one rejection by the auth limiter `limiter` (one of [`LIMITERS`]).
pub fn rate_limited(limiter: &'static str) {
    METRICS.rate_limited.with_label_values(&[limiter]).inc();
}

/// The current exposition text (0.0.4), without refreshing the jobs gauge.
pub fn render() -> String {
    let mut out = Vec::new();
    TextEncoder::new()
        .encode(&METRICS.registry.gather(), &mut out)
        .expect("text encoding does not fail");
    String::from_utf8(out).expect("exposition is UTF-8")
}

/// A bounded `method` label.
fn method_label(method: &http::Method) -> &'static str {
    match *method {
        http::Method::GET => "GET",
        http::Method::POST => "POST",
        http::Method::PUT => "PUT",
        http::Method::PATCH => "PATCH",
        http::Method::DELETE => "DELETE",
        http::Method::HEAD => "HEAD",
        http::Method::OPTIONS => "OPTIONS",
        _ => "OTHER",
    }
}

/// Middleware recording request count and latency. Install on the API router only.
pub async fn track(request: Request, next: Next) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| UNMATCHED.to_owned(), |m| m.as_str().to_owned());
    let method = method_label(request.method());
    let started = Instant::now();
    let response = next.run(request).await;
    let elapsed = started.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();
    METRICS
        .requests
        .with_label_values(&[&route, method, &status])
        .inc();
    METRICS
        .duration
        .with_label_values(&[&route, method])
        .observe(elapsed);
    response
}

struct Scrape {
    pool: PgPool,
    /// When the jobs gauge was last refreshed; the lock also serialises refresh + gather.
    refreshed: Mutex<Option<Instant>>,
}

/// The scrape listener's router: `GET /metrics` and nothing else, no auth, no CORS, no
/// request logging (OBS-2).
pub fn metrics_router(pool: PgPool) -> Router {
    Router::new()
        .route("/metrics", get(scrape))
        .with_state(Arc::new(Scrape {
            pool,
            refreshed: Mutex::new(None),
        }))
}

async fn scrape(State(scrape): State<Arc<Scrape>>) -> Response {
    let mut refreshed = scrape.refreshed.lock().await;
    if refreshed.is_none_or(|at| at.elapsed() >= JOBS_CACHE_TTL) {
        // Keep the stale gauge when the database is slow or down: a scrape must neither
        // fail nor hang on it, and a failed attempt also waits out the TTL.
        match tokio::time::timeout(JOBS_QUERY_TIMEOUT, refresh_jobs(&scrape.pool)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "jobs gauge refresh failed"),
            Err(_) => tracing::warn!("jobs gauge refresh timed out"),
        }
        *refreshed = Some(Instant::now());
    }
    let body = render();
    drop(refreshed);
    match HeaderValue::from_str(CONTENT_TYPE) {
        Ok(ct) => ([(header::CONTENT_TYPE, ct)], body).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// One `GROUP BY` over `jobs`; series that vanished are reset to absent.
async fn refresh_jobs(pool: &PgPool) -> Result<(), sqlx::Error> {
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT kind, status, count(*) FROM jobs GROUP BY kind, status")
            .fetch_all(pool)
            .await?;
    METRICS.jobs_rows.reset();
    for (kind, status, n) in rows {
        METRICS
            .jobs_rows
            .with_label_values(&[&kind, &status])
            .set(n);
    }
    Ok(())
}

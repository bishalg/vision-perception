//! owner: WP-B.13
//!
//! Prometheus listener and API HSTS (OBS-2, OBS-3, OBS-4): the listener is off by default
//! and never on the API port, route labels are templates, no id/e-mail/token reaches the
//! exposition, the histogram has the specified buckets, HSTS is on every response, a scrape
//! writes no request log, and an auth rate-limit rejection is counted. The listener tests run
//! the real `asd-backend serve` binary; the jobs-gauge test needs `DATABASE_URL`.

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use asd_backend::observability::{self, BUCKETS, LIMITERS};
use common::auth::{from_ip, json};
use common::*;
use http::{Method, StatusCode};

const HSTS: &str = "max-age=63072000; includeSubDomains";

// ---- helpers ------------------------------------------------------------------------

/// The value of the sample line `name{labels}` (labels exactly as exposed, alphabetical).
fn sample(text: &str, series: &str) -> Option<f64> {
    text.lines()
        .find_map(|l| l.strip_prefix(series)?.strip_prefix(' ')?.parse().ok())
}

fn free_addr() -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    l.local_addr().expect("addr")
}

/// One HTTP/1.1 request over a raw socket: `(status, head, body)`.
fn raw_http(addr: SocketAddr, method: &str, path: &str) -> std::io::Result<(u16, String, String)> {
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )?;
    let mut out = String::new();
    s.read_to_string(&mut out)?;
    let (head, body) = out.split_once("\r\n\r\n").unwrap_or((&out, ""));
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("status line");
    Ok((status, head.to_owned(), body.to_owned()))
}

/// A running `asd-backend serve`, killed on drop.
struct Server {
    child: Child,
    api: SocketAddr,
    /// Everything the process wrote to stdout (its JSON log), filled by a reader thread.
    log: Arc<Mutex<String>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn serve(metrics: Option<SocketAddr>) -> Server {
    serve_with_log(metrics, "off")
}

fn serve_with_log(metrics: Option<SocketAddr>, rust_log: &str) -> Server {
    let api = free_addr();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_asd-backend"));
    cmd.arg("serve")
        .env("DATABASE_URL", UNREACHABLE_DB_URL)
        .env("ASD_BLOB_BACKEND", "memory")
        .env("ASD_BIND_ADDR", api.to_string())
        .env("RUST_LOG", rust_log)
        .env_remove("ASD_METRICS_BIND_ADDR")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(m) = metrics {
        cmd.env("ASD_METRICS_BIND_ADDR", m.to_string());
    }
    let mut child = cmd.spawn().expect("spawn asd-backend serve");
    let log = Arc::new(Mutex::new(String::new()));
    let mut stdout = child.stdout.take().expect("piped stdout");
    let sink = log.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = stdout.read(&mut buf) {
            if n == 0 {
                break;
            }
            sink.lock()
                .unwrap()
                .push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    });
    let server = Server { child, api, log };
    let deadline = Instant::now() + Duration::from_secs(30);
    while raw_http(api, "GET", "/health").is_err() {
        assert!(Instant::now() < deadline, "serve did not start");
        std::thread::sleep(Duration::from_millis(100));
    }
    server
}

// ---- OBS-2: the listener ------------------------------------------------------------

#[test]
fn listener_is_off_by_default() {
    let config = test_config(UNREACHABLE_DB_URL);
    assert_eq!(config.metrics_bind_addr, None);

    // A real `serve` without ASD_METRICS_BIND_ADDR: nothing listens on the would-be port,
    // and the API port does not answer /metrics either.
    let reserved = free_addr();
    let server = serve(None);
    assert!(
        TcpStream::connect_timeout(&reserved, Duration::from_millis(500)).is_err(),
        "connection must be refused"
    );
    let (status, _, _) = raw_http(server.api, "GET", "/metrics").unwrap();
    assert_eq!(status, 404);
}

#[test]
fn serve_with_the_variable_scrapes_on_the_second_port_only() {
    let metrics = free_addr();
    let server = serve(Some(metrics));
    // Generate one API request so the HTTP series exist.
    let (status, head, _) = raw_http(server.api, "GET", "/health").unwrap();
    assert_eq!(status, 200);
    assert!(head.contains("strict-transport-security"), "{head}");

    let deadline = Instant::now() + Duration::from_secs(10);
    let (status, head, body) = loop {
        match raw_http(metrics, "GET", "/metrics") {
            Ok(r) => break r,
            Err(e) => {
                assert!(Instant::now() < deadline, "metrics listener: {e}");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    assert_eq!(status, 200, "{head}");
    let head = head.to_ascii_lowercase();
    assert!(
        head.contains("content-type: text/plain; version=0.0.4; charset=utf-8"),
        "{head}"
    );
    assert!(!head.contains("access-control-"), "no CORS: {head}");
    assert!(body.contains("asd_backend_build_info{"), "{body}");
    assert!(
        sample(
            &body,
            r#"http_requests_total{method="GET",route="/health",status="200"}"#
        )
        .is_some_and(|n| n >= 1.0),
        "{body}"
    );

    // Nothing but GET /metrics there; and the API port still has no /metrics.
    assert_eq!(raw_http(metrics, "GET", "/health").unwrap().0, 404);
    assert_eq!(raw_http(metrics, "POST", "/metrics").unwrap().0, 405);
    assert_eq!(raw_http(server.api, "GET", "/metrics").unwrap().0, 404);
}

#[tokio::test]
async fn metrics_is_not_served_by_the_api_router() {
    let (status, _, body) = send(router(state_without_db()), get("/metrics")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, _, _) = send(router(state_without_db()), get("/v1/metrics")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---- OBS-3: labels and buckets ----------------------------------------------------

#[tokio::test]
async fn route_label_is_the_template_and_no_id_reaches_the_exposition() {
    let session = uuid::Uuid::new_v4().to_string();
    let org = uuid::Uuid::new_v4().to_string();
    let token = "child-SESSMETRICS-77";
    let email = "metrics.leak@example.org";

    let app = || router(state_without_db());
    let paths = [
        format!("/v1/sessions/{session}"),
        format!("/v1/orgs/{org}/sessions?email={email}"),
        format!("/v1/participants/{token}/summary"),
        format!("/no/such/route/{session}"),
    ];
    for path in &paths {
        let (status, _, _) = send(app(), get(path)).await;
        assert!(status.is_client_error(), "{path}: {status}");
    }
    let (status, _, _) = send(
        app(),
        json(
            Method::POST,
            "/v1/auth/login",
            &serde_json::json!({"email": "", "password": email}),
            None,
        ),
    )
    .await;
    assert!(status.is_client_error());

    let text = observability::render();
    assert!(
        text.contains(r#"route="/v1/sessions/{id}""#),
        "template label missing:\n{text}"
    );
    assert!(
        sample(
            &text,
            r#"http_requests_total{method="GET",route="/v1/sessions/{id}",status="401"}"#
        )
        .is_some_and(|n| n >= 1.0),
        "{text}"
    );
    assert!(
        text.contains(r#"route="unmatched""#),
        "unmatched label missing:\n{text}"
    );
    for needle in [
        session.as_str(),
        org.as_str(),
        token,
        email,
        "SESSMETRICS",
        "metrics.leak",
        "/no/such/route",
    ] {
        assert!(!text.contains(needle), "`{needle}` leaked:\n{text}");
    }
}

#[tokio::test]
async fn histogram_has_the_specified_buckets() {
    let (status, _, _) = send(router(state_without_db()), get("/health")).await;
    assert_eq!(status, StatusCode::OK);
    let text = observability::render();
    assert_eq!(
        BUCKETS,
        [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    );
    let prefix = r#"http_request_duration_seconds_bucket{method="GET",route="/health",le=""#;
    let les: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix(prefix)?.split('"').next())
        .collect();
    assert_eq!(
        les,
        ["0.005", "0.01", "0.025", "0.05", "0.1", "0.25", "0.5", "1", "2.5", "5", "10", "+Inf"],
        "{text}"
    );
    assert!(text.contains("# TYPE http_request_duration_seconds histogram"));
    assert!(sample(
        &text,
        r#"http_request_duration_seconds_count{method="GET",route="/health"}"#
    )
    .is_some_and(|n| n >= 1.0));
    // Closed limiter label set, present from the start; build info is exported.
    for limiter in LIMITERS {
        let series = format!(r#"auth_rate_limited_total{{limiter="{limiter}"}}"#);
        assert!(sample(&text, &series).is_some(), "{series} missing");
    }
    assert!(
        text.contains(r#"asd_backend_build_info{git_sha=""#),
        "{text}"
    );
}

#[tokio::test]
async fn login_rate_limit_rejection_is_counted() {
    let series = r#"auth_rate_limited_total{limiter="login_ip"}"#;
    let before = sample(&observability::render(), series).unwrap_or(0.0);
    let app = router(state_without_db());
    // Empty e-mail: answered 4xx before any database access, but the IP limit counts it.
    let attempt = || {
        from_ip(
            json(
                Method::POST,
                "/v1/auth/login",
                &serde_json::json!({"email": "", "password": ""}),
                None,
            ),
            "203.0.113.77",
        )
    };
    for i in 0..10 {
        let (status, _, _) = send(app.clone(), attempt()).await;
        assert_ne!(status, StatusCode::TOO_MANY_REQUESTS, "attempt {i}");
    }
    let (status, _, _) = send(app.clone(), attempt()).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let after = sample(&observability::render(), series).unwrap();
    assert_eq!(after - before, 1.0, "exactly one rejection counted");
}

// ---- OBS-4: HSTS --------------------------------------------------------------------

#[tokio::test]
async fn hsts_is_on_every_response_including_404() {
    for (path, want) in [
        ("/health", StatusCode::OK),
        ("/no/such/route", StatusCode::NOT_FOUND),
        ("/v1/sessions/abc", StatusCode::UNAUTHORIZED),
    ] {
        let (status, headers, _) = send(router(state_without_db()), get(path)).await;
        assert_eq!(status, want, "{path}");
        let hsts = headers
            .get(http::header::STRICT_TRANSPORT_SECURITY)
            .unwrap_or_else(|| panic!("no HSTS on {path}"))
            .to_str()
            .unwrap();
        assert_eq!(hsts, HSTS, "{path}");
        assert!(!hsts.contains("preload"));
    }
}

// ---- scrape is not logged; jobs gauge -----------------------------------------------

/// Waits until the server's log has a line containing `needle`.
fn wait_for_log(server: &Server, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let log = server.log.lock().unwrap().clone();
        if log.contains(needle) {
            return log;
        }
        assert!(Instant::now() < deadline, "no `{needle}` in log:\n{log}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn scrape_writes_no_request_log() {
    let metrics = free_addr();
    let server = serve_with_log(Some(metrics), "info");
    // Control: an API request produces a request-log line (route template, status).
    assert_eq!(raw_http(server.api, "GET", "/health").unwrap().0, 200);
    wait_for_log(&server, r#""message":"response""#);

    let before = server
        .log
        .lock()
        .unwrap()
        .matches(r#""message":"response""#)
        .count();
    let deadline = Instant::now() + Duration::from_secs(10);
    let scraped = loop {
        match raw_http(metrics, "GET", "/metrics") {
            Ok(r) => break r,
            Err(e) => {
                assert!(Instant::now() < deadline, "metrics listener: {e}");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    assert_eq!(scraped.0, 200);
    // Another API request after the scrape: once its line is in, an earlier scrape line
    // would be too.
    assert_eq!(raw_http(server.api, "GET", "/health").unwrap().0, 200);
    let log = loop {
        let log = wait_for_log(&server, r#""message":"response""#);
        if log.matches(r#""message":"response""#).count() > before {
            break log;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // Not a fixed count: under load the readiness poll in `serve_with_log` can add a late
    // `/health` line. Every response line must be an API `/health` request, and the scrape
    // must not appear anywhere.
    let lines: Vec<&str> = log
        .lines()
        .filter(|l| l.contains(r#""message":"response""#))
        .collect();
    assert!(lines.len() >= 2, "{log}");
    for line in lines {
        assert!(
            line.contains(r#""route":"/health""#),
            "a non-/health request was logged (the scrape?):\n{line}"
        );
    }
    assert!(
        !log.contains("/metrics"),
        "scrape was request-logged:\n{log}"
    );
}

#[test]
fn jobs_gauge_counts_rows_and_is_cached() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let Some(db) = TestDb::migrated().await else {
            return;
        };
        sqlx::query("INSERT INTO jobs (kind, status) VALUES ('norm_aggregate', 'queued')")
            .execute(&db.pool)
            .await
            .unwrap();
        let app = observability::metrics_router(db.pool.clone());
        let (status, headers, body) = text_get(&app, "/metrics").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get(http::header::CONTENT_TYPE).unwrap(),
            "text/plain; version=0.0.4; charset=utf-8"
        );
        let series = r#"jobs_rows{kind="norm_aggregate",status="queued"}"#;
        assert_eq!(sample(&body, series), Some(1.0), "{body}");

        // Cached for 10 s: a new row is not visible on the next scrape.
        sqlx::query("INSERT INTO jobs (kind, status) VALUES ('norm_aggregate', 'queued')")
            .execute(&db.pool)
            .await
            .unwrap();
        let (_, _, again) = text_get(&app, "/metrics").await;
        assert_eq!(sample(&again, series), Some(1.0), "{again}");
        assert_eq!(text_get(&app, "/health").await.0, StatusCode::NOT_FOUND);
        db.cleanup().await;
    });
}

async fn text_get(app: &axum::Router, path: &str) -> (StatusCode, http::HeaderMap, String) {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let response = app.clone().oneshot(get(path)).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

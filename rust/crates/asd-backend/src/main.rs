//! owner: WP-B.1
//!
//! `asd-backend` entry point (DEP-1):
//!
//! - `asd-backend serve`   HTTP API on `ASD_BIND_ADDR` (never runs migrations);
//! - `asd-backend worker`  background job loop (job kinds arrive with WP-B.5);
//! - `asd-backend migrate` applies pending migrations and exits (compose init step).
//!
//! Configuration is environment only; see `rust/.env.example`.

use std::process::ExitCode;

use asd_backend::config::{redact_url, BlobBackend, Config};
use asd_backend::{app, db, telemetry};

const USAGE: &str = "usage: asd-backend <serve|worker|migrate>\n\
\n\
  serve    run the HTTP API (GET /health, /health/ready, /v1/...)\n\
  worker   run the background job worker\n\
  migrate  apply pending database migrations and exit\n\
\n\
Configuration comes from the environment; see rust/.env.example.";

#[derive(Clone, Copy)]
enum Command {
    Serve,
    Worker,
    Migrate,
}

fn main() -> ExitCode {
    let command = match std::env::args().nth(1).as_deref() {
        Some("serve") => Command::Serve,
        Some("worker") => Command::Worker,
        Some("migrate") => Command::Migrate,
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("asd-backend: configuration error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = telemetry::init(&config.log_filter) {
        eprintln!("asd-backend: {e}");
        return ExitCode::from(2);
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "cannot start the async runtime");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async move {
        match command {
            Command::Serve => serve(config).await,
            Command::Worker => worker(config).await,
            Command::Migrate => migrate(config).await,
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "asd-backend exited with an error");
            ExitCode::FAILURE
        }
    }
}

async fn serve(config: Config) -> Result<(), String> {
    tracing::info!(
        bind_addr = %config.bind_addr,
        database = %redact_url(&config.database_url),
        blob_mode = ?config.blob_mode,
        blob_backend = ?config.blob_backend,
        allow_synthetic = config.allow_synthetic,
        "starting api"
    );
    if config.blob_backend == BlobBackend::Memory {
        tracing::warn!("ASD_BLOB_BACKEND=memory: uploaded blobs are lost on restart");
    }
    let pool = db::connect_lazy(&config).map_err(|e| format!("database url: {e}"))?;
    match db::pending_migrations(&pool).await {
        Ok(p) if p.is_empty() => {}
        Ok(p) => tracing::warn!(pending = ?p, "migrations pending; run `asd-backend migrate`"),
        Err(e) => {
            tracing::warn!(error = %e, "database not reachable yet; /health/ready reports it")
        }
    }
    let blobs = app::build_blob_store(&config).await;
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .map_err(|e| format!("bind {}: {e}", config.bind_addr))?;
    let router = app::router(app::AppState::new(pool, blobs, config));
    tracing::info!("api listening");
    // Peer addresses feed the per-IP auth rate limits and audit rows (WP-B.2, AUTH-1).
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| format!("server: {e}"))?;
    tracing::info!("api stopped");
    Ok(())
}

async fn worker(config: Config) -> Result<(), String> {
    let _pool = db::connect_lazy(&config).map_err(|e| format!("database url: {e}"))?;
    // WP-B.5 adds the job loop (`jobs::runner`, `FOR UPDATE SKIP LOCKED` on `jobs`).
    tracing::info!("worker started; no job kinds registered yet");
    shutdown_signal().await;
    tracing::info!("worker stopped");
    Ok(())
}

async fn migrate(config: Config) -> Result<(), String> {
    tracing::info!(database = %redact_url(&config.database_url), "applying migrations");
    let pool = db::connect(&config)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let pending = db::pending_migrations(&pool)
        .await
        .map_err(|e| format!("read migration state: {e}"))?;
    db::migrate(&pool)
        .await
        .map_err(|e| format!("migrate: {e}"))?;
    tracing::info!(applied = ?pending, "migrations up to date");
    pool.close().await;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

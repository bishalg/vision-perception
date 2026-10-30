//! owner: WP-B.1
//!
//! Service configuration from environment variables (12-factor). Every variable is
//! documented in `rust/.env.example`. Secrets never appear in `Debug` output.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

/// How devices upload frames/report blobs (CON-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobMode {
    /// Absolute pre-signed S3 PUT URLs (AWS).
    Presigned,
    /// Relative `/v1/sessions/{id}/blobs/*` URLs served under the device bearer (demo).
    Proxy,
}

/// Which [`crate::blob::BlobStore`] implementation backs the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobBackend {
    S3,
    /// Process memory; for tests and quick local runs only, lost on restart.
    Memory,
}

/// S3 / MinIO settings. Credentials come from the standard AWS chain
/// (`AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`, ECS task role, ...), never from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Settings {
    pub bucket: String,
    pub region: String,
    /// Custom endpoint (MinIO); `None` = AWS.
    pub endpoint_url: Option<String>,
    /// Path-style addressing (required by MinIO).
    pub force_path_style: bool,
}

#[derive(Clone, PartialEq)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub db_max_connections: u32,
    pub db_acquire_timeout: Duration,
    pub blob_mode: BlobMode,
    pub blob_backend: BlobBackend,
    pub s3: S3Settings,
    /// Accept `captureSource: "synthetic"` sessions (CON-13). Never in production.
    pub allow_synthetic: bool,
    /// The console origin allowed by CORS; `None` = no CORS headers at all.
    pub console_origin: Option<String>,
    /// `tracing` filter directive (`RUST_LOG` syntax).
    pub log_filter: String,
    /// Whole-request timeout.
    pub request_timeout: Duration,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{0} is required")]
    Missing(&'static str),
    #[error("{name} has an invalid value: {reason}")]
    Invalid { name: &'static str, reason: String },
}

impl Config {
    /// Reads the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Reads configuration through `lookup` (the environment in production, a map in
    /// tests). Empty values count as unset.
    pub fn from_lookup<F>(lookup: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let get = |key: &str| {
            lookup(key)
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };

        let database_url = get("DATABASE_URL").ok_or(ConfigError::Missing("DATABASE_URL"))?;
        if !(database_url.starts_with("postgres://") || database_url.starts_with("postgresql://")) {
            return Err(ConfigError::Invalid {
                name: "DATABASE_URL",
                reason: "must start with postgres:// or postgresql://".into(),
            });
        }

        let bind_addr = parse_or(
            &get,
            "ASD_BIND_ADDR",
            "0.0.0.0:8000".parse().expect("valid"),
        )?;
        let db_max_connections = parse_or(&get, "ASD_DB_MAX_CONNECTIONS", 10u32)?;
        if db_max_connections == 0 {
            return Err(invalid("ASD_DB_MAX_CONNECTIONS", "must be at least 1"));
        }
        let db_acquire_timeout =
            Duration::from_millis(parse_or(&get, "ASD_DB_ACQUIRE_TIMEOUT_MS", 5_000u64)?);
        let request_timeout =
            Duration::from_secs(parse_or(&get, "ASD_REQUEST_TIMEOUT_SECS", 30u64)?);

        let blob_mode = match get("BLOB_MODE").as_deref() {
            None | Some("proxy") => BlobMode::Proxy,
            Some("presigned") => BlobMode::Presigned,
            Some(other) => {
                return Err(invalid(
                    "BLOB_MODE",
                    format!("`{other}` (expected presigned|proxy)"),
                ))
            }
        };
        let blob_backend = match get("ASD_BLOB_BACKEND").as_deref() {
            None | Some("s3") => BlobBackend::S3,
            Some("memory") => BlobBackend::Memory,
            Some(other) => {
                return Err(invalid(
                    "ASD_BLOB_BACKEND",
                    format!("`{other}` (expected s3|memory)"),
                ))
            }
        };
        if blob_mode == BlobMode::Presigned && blob_backend == BlobBackend::Memory {
            // A memory store cannot hand out URLs a device could reach.
            return Err(invalid("BLOB_MODE", "presigned needs ASD_BLOB_BACKEND=s3"));
        }

        let s3 = S3Settings {
            bucket: get("S3_BUCKET").unwrap_or_else(|| "curv-asd-sessions".into()),
            region: get("S3_REGION")
                .or_else(|| get("AWS_REGION"))
                .unwrap_or_else(|| "us-east-1".into()),
            endpoint_url: get("S3_ENDPOINT_URL"),
            force_path_style: parse_bool(&get, "S3_FORCE_PATH_STYLE", false)?,
        };

        let console_origin = get("ASD_CONSOLE_ORIGIN");
        if let Some(origin) = &console_origin {
            // One explicit origin, never `*` (02 §7, trap 13).
            let scheme_ok = origin.starts_with("https://") || origin.starts_with("http://");
            if !scheme_ok
                || origin.ends_with('/')
                || origin.contains(['*', ',', ' '])
                || http::HeaderValue::from_str(origin).is_err()
            {
                return Err(invalid(
                    "ASD_CONSOLE_ORIGIN",
                    format!("`{origin}` (expected one origin like https://console.example.org)"),
                ));
            }
        }

        Ok(Self {
            bind_addr,
            database_url,
            db_max_connections,
            db_acquire_timeout,
            blob_mode,
            blob_backend,
            s3,
            allow_synthetic: parse_bool(&get, "ASD_ALLOW_SYNTHETIC", false)?,
            console_origin,
            log_filter: get("RUST_LOG").unwrap_or_else(|| "info".into()),
            request_timeout,
        })
    }
}

/// `DATABASE_URL` with the password replaced, for logs and `Debug`.
pub fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return "[redacted]".into();
    };
    match rest.rsplit_once('@') {
        Some((userinfo, host)) => {
            let user = userinfo.split(':').next().unwrap_or("");
            format!("{scheme}://{user}:***@{host}")
        }
        None => url.to_owned(),
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bind_addr", &self.bind_addr)
            .field("database_url", &redact_url(&self.database_url))
            .field("db_max_connections", &self.db_max_connections)
            .field("db_acquire_timeout", &self.db_acquire_timeout)
            .field("blob_mode", &self.blob_mode)
            .field("blob_backend", &self.blob_backend)
            .field("s3", &self.s3)
            .field("allow_synthetic", &self.allow_synthetic)
            .field("console_origin", &self.console_origin)
            .field("log_filter", &self.log_filter)
            .field("request_timeout", &self.request_timeout)
            .finish()
    }
}

fn invalid(name: &'static str, reason: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        name,
        reason: reason.into(),
    }
}

fn parse_or<T, G>(get: &G, name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
    G: Fn(&str) -> Option<String>,
{
    match get(name) {
        None => Ok(default),
        Some(v) => v
            .parse()
            .map_err(|e: T::Err| invalid(name, format!("`{v}`: {e}"))),
    }
}

fn parse_bool<G>(get: &G, name: &'static str, default: bool) -> Result<bool, ConfigError>
where
    G: Fn(&str) -> Option<String>,
{
    match get(name).map(|v| v.to_ascii_lowercase()).as_deref() {
        None => Ok(default),
        Some("1" | "true" | "yes" | "on") => Ok(true),
        Some("0" | "false" | "no" | "off") => Ok(false),
        Some(other) => Err(invalid(name, format!("`{other}` is not a boolean"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const URL: &str = "postgres://asd:s3cret@db:5432/asd";

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|k| map.get(k).cloned())
    }

    #[test]
    fn defaults_with_only_database_url() {
        let c = cfg(&[("DATABASE_URL", URL)]).unwrap();
        assert_eq!(c.bind_addr, "0.0.0.0:8000".parse().unwrap());
        assert_eq!(c.database_url, URL);
        assert_eq!(c.db_max_connections, 10);
        assert_eq!(c.db_acquire_timeout, Duration::from_secs(5));
        assert_eq!(c.blob_mode, BlobMode::Proxy);
        assert_eq!(c.blob_backend, BlobBackend::S3);
        assert_eq!(c.s3.bucket, "curv-asd-sessions");
        assert_eq!(c.s3.region, "us-east-1");
        assert_eq!(c.s3.endpoint_url, None);
        assert!(!c.s3.force_path_style);
        assert!(
            !c.allow_synthetic,
            "synthetic sessions are off by default (CON-13)"
        );
        assert_eq!(c.console_origin, None);
        assert_eq!(c.log_filter, "info");
        assert_eq!(c.request_timeout, Duration::from_secs(30));
    }

    #[test]
    fn database_url_is_required_and_must_be_postgres() {
        assert_eq!(cfg(&[]).unwrap_err(), ConfigError::Missing("DATABASE_URL"));
        assert_eq!(
            cfg(&[("DATABASE_URL", "  ")]).unwrap_err(),
            ConfigError::Missing("DATABASE_URL")
        );
        assert!(matches!(
            cfg(&[("DATABASE_URL", "sqlite://x.db")]).unwrap_err(),
            ConfigError::Invalid {
                name: "DATABASE_URL",
                ..
            }
        ));
        assert!(cfg(&[("DATABASE_URL", "postgresql://a@b/c")]).is_ok());
    }

    #[test]
    fn every_variable_is_read() {
        let c = cfg(&[
            ("DATABASE_URL", URL),
            ("ASD_BIND_ADDR", "127.0.0.1:9000"),
            ("ASD_DB_MAX_CONNECTIONS", "3"),
            ("ASD_DB_ACQUIRE_TIMEOUT_MS", "250"),
            ("ASD_REQUEST_TIMEOUT_SECS", "60"),
            ("BLOB_MODE", "presigned"),
            ("ASD_BLOB_BACKEND", "s3"),
            ("S3_BUCKET", "b"),
            ("AWS_REGION", "eu-west-2"),
            ("S3_ENDPOINT_URL", "http://minio:9000"),
            ("S3_FORCE_PATH_STYLE", "true"),
            ("ASD_ALLOW_SYNTHETIC", "1"),
            ("ASD_CONSOLE_ORIGIN", "https://console.example.org"),
            ("RUST_LOG", "debug,sqlx=warn"),
        ])
        .unwrap();
        assert_eq!(c.bind_addr, "127.0.0.1:9000".parse().unwrap());
        assert_eq!(c.db_max_connections, 3);
        assert_eq!(c.db_acquire_timeout, Duration::from_millis(250));
        assert_eq!(c.request_timeout, Duration::from_secs(60));
        assert_eq!(c.blob_mode, BlobMode::Presigned);
        assert_eq!(
            c.s3,
            S3Settings {
                bucket: "b".into(),
                region: "eu-west-2".into(),
                endpoint_url: Some("http://minio:9000".into()),
                force_path_style: true,
            }
        );
        assert!(c.allow_synthetic);
        assert_eq!(
            c.console_origin.as_deref(),
            Some("https://console.example.org")
        );
        assert_eq!(c.log_filter, "debug,sqlx=warn");
    }

    #[test]
    fn s3_region_prefers_s3_region_over_aws_region() {
        let c = cfg(&[
            ("DATABASE_URL", URL),
            ("S3_REGION", "a"),
            ("AWS_REGION", "b"),
        ])
        .unwrap();
        assert_eq!(c.s3.region, "a");
    }

    #[test]
    fn invalid_values_name_the_variable() {
        for (name, value) in [
            ("ASD_BIND_ADDR", "not-an-addr"),
            ("ASD_DB_MAX_CONNECTIONS", "-1"),
            ("ASD_DB_MAX_CONNECTIONS", "0"),
            ("ASD_DB_ACQUIRE_TIMEOUT_MS", "soon"),
            ("BLOB_MODE", "ftp"),
            ("ASD_BLOB_BACKEND", "disk"),
            ("ASD_ALLOW_SYNTHETIC", "maybe"),
            ("S3_FORCE_PATH_STYLE", "2"),
            ("ASD_CONSOLE_ORIGIN", "*"),
            ("ASD_CONSOLE_ORIGIN", "console.example.org"),
            (
                "ASD_CONSOLE_ORIGIN",
                "https://a.example.org,https://b.example.org",
            ),
            ("ASD_CONSOLE_ORIGIN", "https://console.example.org/"),
        ] {
            let err = cfg(&[("DATABASE_URL", URL), (name, value)]).unwrap_err();
            match err {
                ConfigError::Invalid { name: n, .. } => assert_eq!(n, name, "{value}"),
                other => panic!("{name}={value}: {other:?}"),
            }
        }
    }

    #[test]
    fn presigned_mode_needs_s3_backend() {
        let err = cfg(&[
            ("DATABASE_URL", URL),
            ("BLOB_MODE", "presigned"),
            ("ASD_BLOB_BACKEND", "memory"),
        ])
        .unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "BLOB_MODE",
                ..
            }
        ));
        assert!(cfg(&[("DATABASE_URL", URL), ("ASD_BLOB_BACKEND", "memory")]).is_ok());
    }

    #[test]
    fn booleans_accept_common_spellings() {
        for (v, want) in [
            ("TRUE", true),
            ("yes", true),
            ("on", true),
            ("0", false),
            ("off", false),
        ] {
            let c = cfg(&[("DATABASE_URL", URL), ("ASD_ALLOW_SYNTHETIC", v)]).unwrap();
            assert_eq!(c.allow_synthetic, want, "{v}");
        }
    }

    #[test]
    fn debug_never_prints_the_database_password() {
        let c = cfg(&[("DATABASE_URL", URL)]).unwrap();
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("s3cret"), "{dbg}");
        assert!(dbg.contains("postgres://asd:***@db:5432/asd"), "{dbg}");
    }

    #[test]
    fn redact_url_handles_odd_shapes() {
        assert_eq!(redact_url("postgres://u:p@h/d"), "postgres://u:***@h/d");
        assert_eq!(redact_url("postgres://u@h/d"), "postgres://u:***@h/d");
        assert_eq!(redact_url("postgres://h/d"), "postgres://h/d");
        assert_eq!(redact_url("postgres://u:p@ss@h/d"), "postgres://u:***@h/d");
        assert_eq!(redact_url("garbage"), "[redacted]");
    }
}

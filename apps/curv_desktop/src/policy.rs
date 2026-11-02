//! Pure policy functions: server URL validation, the navigation allow-list and the
//! stored config format. No Tauri types here, so every rule is unit tested.

use serde::{Deserialize, Serialize};
use url::Url;

/// Hosts for which plain `http` is accepted (a console on the same machine).
const LOOPBACK_HOSTS: [&str; 2] = ["localhost", "127.0.0.1"];

/// Why a server URL was refused. The message is shown on the setup page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlError {
    Empty,
    Unparseable,
    SchemeNotAllowed,
    HttpOnlyForLocalhost,
    MissingHost,
    CredentialsNotAllowed,
}

impl std::fmt::Display for UrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            UrlError::Empty => "Enter the clinic server address.",
            UrlError::Unparseable => {
                "That is not a valid address (example: https://clinic.example.org)."
            }
            UrlError::SchemeNotAllowed => "The address must start with https://.",
            UrlError::HttpOnlyForLocalhost => {
                "Plain http:// is allowed only for localhost or 127.0.0.1. Use https://."
            }
            UrlError::MissingHost => "The address has no server name.",
            UrlError::CredentialsNotAllowed => "Do not put a user name or password in the address.",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for UrlError {}

/// Validates and normalises a clinic server URL typed by the user.
///
/// Rules: `https` for any host; `http` only for `localhost` / `127.0.0.1`; a host is
/// required; no user info. Query and fragment are dropped, the path is kept (a console
/// behind a reverse proxy may live under a prefix).
pub fn validate_server_url(input: &str) -> Result<Url, UrlError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(UrlError::Empty);
    }
    let mut url = Url::parse(trimmed).map_err(|_| UrlError::Unparseable)?;
    match url.scheme() {
        "https" => {}
        "http" => {
            let host = url.host_str().unwrap_or_default();
            if !LOOPBACK_HOSTS.contains(&host) {
                return Err(UrlError::HttpOnlyForLocalhost);
            }
        }
        _ => return Err(UrlError::SchemeNotAllowed),
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(UrlError::MissingHost);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlError::CredentialsNotAllowed);
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

/// What the shell does with a navigation request in the console window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
    /// Load it in the console window.
    Allow,
    /// Do not load it; open it in the system browser instead.
    OpenExternal,
    /// Do not load it anywhere.
    Block,
}

/// Same origin in the HTML sense (scheme, host, effective port). Written out rather than
/// `Url::origin()` so that non-special schemes such as `tauri://localhost` compare by
/// value instead of being opaque.
pub fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme()
        && a.host_str().map(str::to_ascii_lowercase) == b.host_str().map(str::to_ascii_lowercase)
        && a.port_or_known_default() == b.port_or_known_default()
}

/// The navigation allow-list for the console window: only the configured server origin
/// loads in the app. `blob:` URLs count as the origin that created them (console CSV/ZIP
/// downloads). Other web and mail links go to the system browser; everything else
/// (`file:`, `data:`, `javascript:`, custom schemes, ...) is blocked.
pub fn classify_navigation(server: &Url, target: &Url) -> Navigation {
    match target.scheme() {
        "http" | "https" if same_origin(server, target) => Navigation::Allow,
        "http" | "https" | "mailto" => Navigation::OpenExternal,
        "blob" => match Url::parse(target.path()) {
            Ok(inner) if same_origin(server, &inner) => Navigation::Allow,
            _ => Navigation::Block,
        },
        // Inert documents an engine may "navigate" a frame to; they carry no content.
        "about" if matches!(target.path(), "blank" | "srcdoc") => Navigation::Allow,
        _ => Navigation::Block,
    }
}

/// Whether a URL belongs to the bundled local assets (the setup page). Tauri serves them
/// from `tauri://localhost` (macOS, Linux) or `http(s)://tauri.localhost` (Windows).
pub fn is_local_app_url(target: &Url) -> bool {
    match target.scheme() {
        "tauri" => target.host_str() == Some("localhost"),
        "http" | "https" => target.host_str() == Some("tauri.localhost"),
        _ => false,
    }
}

/// The on-disk config, `<app config dir>/server.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredConfig {
    pub server_url: String,
}

/// Reads a stored config. A missing, corrupt or no-longer-valid entry yields `None`, so
/// the app falls back to the setup page instead of loading something unchecked.
pub fn parse_config(json: &str) -> Option<Url> {
    let cfg: StoredConfig = serde_json::from_str(json).ok()?;
    validate_server_url(&cfg.server_url).ok()
}

pub fn serialize_config(url: &Url) -> String {
    let cfg = StoredConfig {
        server_url: url.to_string(),
    };
    // A struct with one String field cannot fail to serialise.
    serde_json::to_string_pretty(&cfg).expect("config serialises")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn accepts_https_and_normalises() {
        let url = validate_server_url("  https://clinic.example.org  ").unwrap();
        assert_eq!(url.as_str(), "https://clinic.example.org/");
        let url = validate_server_url("https://clinic.example.org:8443/console?x=1#frag").unwrap();
        assert_eq!(url.as_str(), "https://clinic.example.org:8443/console");
    }

    #[test]
    fn accepts_http_only_for_loopback() {
        assert!(validate_server_url("http://localhost:3000").is_ok());
        assert!(validate_server_url("http://127.0.0.1:3000/").is_ok());
        assert!(validate_server_url("http://LOCALHOST:3000").is_ok());
        for bad in [
            "http://clinic.example.org",
            "http://192.168.1.10:3000",
            "http://localhost.evil.example",
            "http://127.0.0.1.nip.io",
            "http://10.0.0.1",
        ] {
            assert_eq!(
                validate_server_url(bad),
                Err(UrlError::HttpOnlyForLocalhost),
                "{bad}"
            );
        }
    }

    #[test]
    fn rejects_other_schemes_and_garbage() {
        assert_eq!(validate_server_url(""), Err(UrlError::Empty));
        assert_eq!(validate_server_url("   "), Err(UrlError::Empty));
        assert_eq!(
            validate_server_url("clinic.example.org"),
            Err(UrlError::Unparseable)
        );
        for bad in [
            "ftp://clinic.example.org",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
            "tauri://localhost",
            "ws://localhost:3000",
        ] {
            assert_eq!(
                validate_server_url(bad),
                Err(UrlError::SchemeNotAllowed),
                "{bad}"
            );
        }
    }

    #[test]
    fn rejects_credentials() {
        assert_eq!(
            validate_server_url("https://user:pw@clinic.example.org"),
            Err(UrlError::CredentialsNotAllowed)
        );
        assert_eq!(
            validate_server_url("https://user@clinic.example.org"),
            Err(UrlError::CredentialsNotAllowed)
        );
    }

    #[test]
    fn navigation_allows_only_the_server_origin() {
        let server = u("https://clinic.example.org/");
        assert_eq!(
            classify_navigation(&server, &u("https://clinic.example.org/patients/42?tab=1")),
            Navigation::Allow
        );
        assert_eq!(
            classify_navigation(&server, &u("https://clinic.example.org:443/login")),
            Navigation::Allow
        );
        assert_eq!(
            classify_navigation(&server, &u("https://CLINIC.example.org/")),
            Navigation::Allow
        );
        // Different scheme, port or host: not the same origin.
        for ext in [
            "http://clinic.example.org/",
            "https://clinic.example.org:8443/",
            "https://evil.example.org/",
            "https://clinic.example.org.evil.example/",
            "https://docs.example.com/help",
            "mailto:support@example.org",
        ] {
            assert_eq!(
                classify_navigation(&server, &u(ext)),
                Navigation::OpenExternal,
                "{ext}"
            );
        }
    }

    #[test]
    fn navigation_blocks_dangerous_schemes() {
        let server = u("https://clinic.example.org/");
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "tauri://localhost/index.html",
            "ftp://clinic.example.org/",
            "about:config",
            "blob:https://evil.example.org/2c1b",
            "blob:not-a-url",
        ] {
            assert_eq!(
                classify_navigation(&server, &u(bad)),
                Navigation::Block,
                "{bad}"
            );
        }
    }

    #[test]
    fn navigation_allows_same_origin_blobs_and_inert_frames() {
        let server = u("https://clinic.example.org/");
        assert_eq!(
            classify_navigation(&server, &u("blob:https://clinic.example.org/2c1b-77")),
            Navigation::Allow
        );
        assert_eq!(
            classify_navigation(&server, &u("about:blank")),
            Navigation::Allow
        );
    }

    #[test]
    fn localhost_server_origin_includes_port() {
        let server = validate_server_url("http://localhost:3000").unwrap();
        assert_eq!(
            classify_navigation(&server, &u("http://localhost:3000/cases")),
            Navigation::Allow
        );
        assert_eq!(
            classify_navigation(&server, &u("http://localhost:8000/v1/health")),
            Navigation::OpenExternal
        );
    }

    #[test]
    fn local_app_urls() {
        assert!(is_local_app_url(&u("tauri://localhost/index.html")));
        assert!(is_local_app_url(&u("http://tauri.localhost/index.html")));
        assert!(is_local_app_url(&u("https://tauri.localhost/")));
        assert!(!is_local_app_url(&u("tauri://evil/index.html")));
        assert!(!is_local_app_url(&u("https://clinic.example.org/")));
        assert!(!is_local_app_url(&u(
            "https://tauri.localhost.evil.example/"
        )));
    }

    #[test]
    fn config_round_trip_and_rejects_invalid_entries() {
        let url = validate_server_url("https://clinic.example.org").unwrap();
        assert_eq!(parse_config(&serialize_config(&url)), Some(url));
        assert_eq!(parse_config("not json"), None);
        assert_eq!(
            parse_config(r#"{"server_url":"http://evil.example"}"#),
            None
        );
        assert_eq!(parse_config(r#"{"other":1}"#), None);
    }
}

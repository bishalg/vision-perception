//! owner: WP-B.2
//!
//! Authentication and authorisation for both planes (02 ADR-B4):
//!
//! - user plane: Argon2id passwords ([`password`], AUTH-1), EdDSA access tokens ([`jwt`],
//!   AUTH-2) and opaque rotating refresh-token families ([`refresh`], AUTH-2);
//! - device plane: `dvc_` bearer tokens ([`device_token`], AUTH-3) issued against enrol
//!   codes (CON-11);
//! - RBAC: one matrix table ([`rbac`], AUTH-4) applied by the [`extractors::Require`]
//!   extractor.
//!
//! Secrets (passwords, refresh tokens, device tokens, enrol codes) are stored only as
//! hashes and never logged; the signing key comes from `ASD_JWT_SIGNING_KEY`.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::config::ConfigError;

pub mod device_token;
pub mod extractors;
pub mod jwt;
pub mod password;
pub mod rbac;
pub mod refresh;

/// Auth settings, part of [`crate::config::Config`]. `Debug` never prints key material.
#[derive(Clone, PartialEq, Default)]
pub struct AuthConfig {
    /// `ASD_JWT_SIGNING_KEY`: Ed25519 key as PKCS#8 PEM (`asd-cli bundle keygen`) or the
    /// base64 32-byte seed. Must differ from the bundle signing key (AUTH-2). Unset = the
    /// user plane answers 503 (no generated default outside tests).
    pub jwt: Option<jwt::JwtKeys>,
    /// `ASD_TRUST_PROXY_HEADERS`: take the client IP from `CF-Connecting-IP` / the last
    /// `X-Forwarded-For` hop (Cloudflare Tunnel, ALB) for rate limits and audit rows
    /// (trap 12). Off by default: a direct client could forge them.
    pub trust_proxy_headers: bool,
}

impl AuthConfig {
    /// Reads the auth variables through `get` (trimmed, empty = unset).
    pub fn from_lookup<G>(get: &G) -> Result<Self, ConfigError>
    where
        G: Fn(&str) -> Option<String>,
    {
        let jwt =
            match get("ASD_JWT_SIGNING_KEY") {
                None => None,
                Some(raw) => Some(jwt::JwtKeys::from_key_str(&raw).map_err(|reason| {
                    ConfigError::Invalid {
                        name: "ASD_JWT_SIGNING_KEY",
                        reason,
                    }
                })?),
            };
        if let (Some(keys), Some(bundle)) = (&jwt, get("ASD_BUNDLE_SIGNING_KEY")) {
            // AUTH-2: a separate key from config-bundle signing (CON-6).
            if let Ok(bundle) = asd_contract::signing::parse_signing_key(&bundle) {
                if bundle.verifying_key().to_bytes() == keys.public_key_bytes() {
                    return Err(ConfigError::Invalid {
                        name: "ASD_JWT_SIGNING_KEY",
                        reason: "must differ from ASD_BUNDLE_SIGNING_KEY".into(),
                    });
                }
            }
        }
        let trust_proxy_headers = match get("ASD_TRUST_PROXY_HEADERS")
            .map(|v| v.to_ascii_lowercase())
            .as_deref()
        {
            None | Some("0" | "false" | "no" | "off") => false,
            Some("1" | "true" | "yes" | "on") => true,
            Some(other) => {
                return Err(ConfigError::Invalid {
                    name: "ASD_TRUST_PROXY_HEADERS",
                    reason: format!("`{other}` is not a boolean"),
                })
            }
        };
        Ok(Self {
            jwt,
            trust_proxy_headers,
        })
    }
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("jwt", &self.jwt)
            .field("trust_proxy_headers", &self.trust_proxy_headers)
            .finish()
    }
}

// ---- token primitives ------------------------------------------------------------------

/// `N` bytes from the OS CSPRNG. Panics only when the OS has no randomness source, in
/// which case no token could be issued safely anyway.
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).expect("OS randomness is available");
    buf
}

/// SHA-256 of a secret; the only form in which tokens and codes are stored.
pub fn sha256(secret: &[u8]) -> [u8; 32] {
    Sha256::digest(secret).into()
}

/// `prefix` + base64url (no padding) of 32 random bytes, with its SHA-256.
pub(crate) fn opaque_token(prefix: &str) -> (String, [u8; 32]) {
    let token = format!("{prefix}{}", URL_SAFE_NO_PAD.encode(random_bytes::<32>()));
    let hash = sha256(token.as_bytes());
    (token, hash)
}

/// True when `token` is `prefix` + exactly 43 base64url characters (32 bytes).
pub(crate) fn is_opaque_token(token: &str, prefix: &str) -> bool {
    token.strip_prefix(prefix).is_some_and(|rest| {
        rest.len() == 43
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

// ---- enrol codes (CON-11) --------------------------------------------------------------

/// Crockford base32 without I, L, O, U: unambiguous when read aloud or typed.
const ENROL_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A new enrol code `XXXX-XXXX-XXXX` (60 bits). Shown once; stored as
/// [`enrol_code_hash`].
pub fn new_enrol_code() -> String {
    let bytes = random_bytes::<12>();
    let chars: Vec<char> = bytes
        .iter()
        .map(|b| char::from(ENROL_ALPHABET[usize::from(b & 31)]))
        .collect();
    chars
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// The canonical form people may type a code in: upper case, without spaces or dashes.
pub fn normalize_enrol_code(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect()
}

/// SHA-256 of the normalised code (`enrol_codes.code_hash`).
pub fn enrol_code_hash(code: &str) -> [u8; 32] {
    sha256(normalize_enrol_code(code).as_bytes())
}

// ---- rate limiting (AUTH-1, AUTH-3) ----------------------------------------------------

/// Fixed-window counter per key: at most `limit` hits per `window`. In memory, per
/// process; with more than one API replica the effective limit is `limit x replicas`
/// (WAF rate rules cover the AWS deployment, DEP-3).
#[derive(Debug)]
pub struct RateLimiter<K> {
    limit: u32,
    window: Duration,
    hits: Mutex<HashMap<K, (Instant, u32)>>,
}

impl<K: Eq + Hash + Clone> RateLimiter<K> {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// `AUTH-1`: 10 per minute.
    pub fn per_minute(limit: u32) -> Self {
        Self::new(limit, Duration::from_secs(60))
    }

    /// Counts one hit for `key`. `Err(retry_after_secs)` when over the limit.
    pub fn check(&self, key: &K) -> Result<(), u64> {
        self.check_at(key, Instant::now())
    }

    pub fn check_at(&self, key: &K, now: Instant) -> Result<(), u64> {
        let mut hits = self.hits.lock().unwrap_or_else(|p| p.into_inner());
        if hits.len() > 10_000 {
            // Bound memory under a spray of distinct keys.
            hits.retain(|_, (start, _)| now.duration_since(*start) < self.window);
        }
        let entry = hits.entry(key.clone()).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        if entry.1 >= self.limit {
            let left = self.window.saturating_sub(now.duration_since(entry.0));
            return Err(left.as_secs().max(1));
        }
        entry.1 += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn opaque_tokens_are_prefixed_32_byte_base64url_and_unique() {
        let mut seen = HashSet::new();
        for _ in 0..100 {
            let (token, hash) = opaque_token("dvc_");
            assert!(is_opaque_token(&token, "dvc_"), "{token}");
            assert_eq!(hash, sha256(token.as_bytes()));
            assert!(seen.insert(token));
        }
        assert!(!is_opaque_token("dvc_abc", "dvc_"));
        assert!(!is_opaque_token(&format!("rt_{}", "A".repeat(43)), "dvc_"));
        assert!(!is_opaque_token(&format!("dvc_{}", "+".repeat(43)), "dvc_"));
    }

    #[test]
    fn enrol_codes_are_grouped_unambiguous_and_normalise() {
        let code = new_enrol_code();
        assert_eq!(code.len(), 14, "{code}");
        assert_eq!(code.matches('-').count(), 2);
        assert!(code
            .chars()
            .filter(|c| *c != '-')
            .all(|c| ENROL_ALPHABET.contains(&(c as u8))));
        assert!(!code.contains(['I', 'L', 'O', 'U']));
        let typed = format!(" {} ", code.to_lowercase().replace('-', " "));
        assert_eq!(enrol_code_hash(&typed), enrol_code_hash(&code));
        assert_ne!(enrol_code_hash("AAAA"), enrol_code_hash("AAAB"));
        assert_ne!(new_enrol_code(), new_enrol_code());
    }

    #[test]
    fn rate_limiter_allows_limit_per_window_per_key() {
        let rl = RateLimiter::per_minute(10);
        let t0 = Instant::now();
        for _ in 0..10 {
            assert!(rl.check_at(&"a", t0).is_ok());
        }
        let retry = rl.check_at(&"a", t0 + Duration::from_secs(15)).unwrap_err();
        assert_eq!(retry, 45);
        assert!(rl.check_at(&"b", t0).is_ok(), "keys are independent");
        assert!(
            rl.check_at(&"a", t0 + Duration::from_secs(60)).is_ok(),
            "new window"
        );
        let short = RateLimiter::new(1, Duration::from_millis(300));
        assert!(short.check_at(&"x", t0).is_ok());
        assert_eq!(
            short.check_at(&"x", t0).unwrap_err(),
            1,
            "Retry-After is at least one second"
        );
    }

    #[test]
    fn auth_config_reads_and_validates() {
        let pem = asd_contract::signing::signing_key_to_pkcs8_pem(
            &asd_contract::signing::signing_key_from_seed([1u8; 32]),
        )
        .unwrap();
        let cfg = |pairs: &[(&str, &str)]| {
            let map: HashMap<String, String> = pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            AuthConfig::from_lookup(&|k: &str| map.get(k).cloned())
        };
        let none = cfg(&[]).unwrap();
        assert!(none.jwt.is_none() && !none.trust_proxy_headers);

        let c = cfg(&[
            ("ASD_JWT_SIGNING_KEY", &pem),
            ("ASD_TRUST_PROXY_HEADERS", "true"),
        ])
        .unwrap();
        assert!(c.jwt.is_some() && c.trust_proxy_headers);
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("PRIVATE"), "{dbg}");

        let seed = base64::engine::general_purpose::STANDARD.encode([2u8; 32]);
        assert!(cfg(&[("ASD_JWT_SIGNING_KEY", &seed)])
            .unwrap()
            .jwt
            .is_some());

        for (name, value) in [
            ("ASD_JWT_SIGNING_KEY", "not a key"),
            ("ASD_TRUST_PROXY_HEADERS", "maybe"),
        ] {
            match cfg(&[(name, value)]).unwrap_err() {
                ConfigError::Invalid { name: n, .. } => assert_eq!(n, name),
                other => panic!("{other:?}"),
            }
        }
        let same = cfg(&[
            ("ASD_JWT_SIGNING_KEY", &pem),
            ("ASD_BUNDLE_SIGNING_KEY", &pem),
        ]);
        assert!(same.is_err(), "JWT and bundle keys must differ");
    }
}

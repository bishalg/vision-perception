//! owner: WP-I.7
//!
//! `integration_destinations` (INTG-1..5): the stored row, the per-kind config rules
//! (INTG-3), and the write-only secret (INTG-2).
//!
//! The secret column holds, per kind, the webhook signing secret (`whsec_...`, DLV-2) or
//! the 32-byte seed of the SFTP destination's Ed25519 key (INTG-5). Both are sealed with
//! the org data key, AAD `"integration_destinations:secret_ct:{org}:{id}"` (KEY-3), and
//! never leave this module except as bytes handed to a channel. Reads expose
//! `has_secret` and the 8-hex fingerprint only.

use asd_contract::export::{
    DeliveryTrigger, ExportMode, Integration, IntegrationConfig, IntegrationEnvironment,
    IntegrationKind, IntegrationStatus, MAX_INTEGRATION_NAME_CHARS, WEBHOOK_SECRET_PREFIX,
};
use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgExecutor;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{netguard, ssh_keys, ErrorClass};
use crate::crypto::{Aad, CryptoError, Keyring};
use crate::error::{AppError, AppResult};
use crate::export_build;
use crate::problem::InvalidParam;
use crate::read::cursor;

/// An `integration_destinations` row. `Debug` hides the ciphertext.
#[derive(Clone)]
pub struct DestinationRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub kind: IntegrationKind,
    pub name: String,
    /// The stored config; rows written before a rule existed may not parse (then
    /// [`config`](Self::config) is the default and deliveries fail `invalid-config`).
    pub config_raw: Value,
    pub status: IntegrationStatus,
    pub environment: IntegrationEnvironment,
    pub secret_ct: Option<Vec<u8>>,
    pub secret_key_id: Option<Uuid>,
    pub secret_fingerprint: Option<String>,
    pub ssh_public_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_test_at: Option<DateTime<Utc>>,
    pub last_test_ok: Option<bool>,
    pub last_test_error: Option<String>,
}

impl std::fmt::Debug for DestinationRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DestinationRow")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("has_secret", &self.secret_ct.is_some())
            .finish_non_exhaustive()
    }
}

fn stored<T>(what: &str, v: Option<T>) -> AppResult<T> {
    v.ok_or_else(|| AppError::Internal(format!("stored destination {what} is not in the contract")))
}

/// One destination by id (any org; callers check the org).
pub async fn load<'e>(db: impl PgExecutor<'e>, id: Uuid) -> AppResult<Option<DestinationRow>> {
    let Some(r) = sqlx::query!(
        r#"SELECT id, org_id, kind, name, config, status, environment, secret_ct,
                  secret_key_id, secret_fingerprint, ssh_public_key, created_at, updated_at,
                  last_test_at, last_test_ok, last_test_error
           FROM integration_destinations WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(DestinationRow {
        id: r.id,
        org_id: r.org_id,
        kind: stored("kind", IntegrationKind::parse(&r.kind))?,
        name: r.name,
        config_raw: r.config,
        status: stored("status", IntegrationStatus::parse(&r.status))?,
        environment: stored("environment", IntegrationEnvironment::parse(&r.environment))?,
        secret_ct: r.secret_ct,
        secret_key_id: r.secret_key_id,
        secret_fingerprint: r.secret_fingerprint,
        ssh_public_key: r.ssh_public_key,
        created_at: r.created_at,
        updated_at: r.updated_at,
        last_test_at: r.last_test_at,
        last_test_ok: r.last_test_ok,
        last_test_error: r.last_test_error,
    }))
}

impl DestinationRow {
    pub fn config(&self) -> IntegrationConfig {
        serde_json::from_value(self.config_raw.clone()).unwrap_or_default()
    }

    /// The `Integration` resource: no secret, no ciphertext.
    pub fn to_dto(&self) -> Integration {
        Integration {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            kind: self.kind,
            name: self.name.clone(),
            environment: self.environment,
            status: self.status,
            config: self.config(),
            has_secret: self.secret_ct.is_some(),
            secret_fingerprint: self.secret_fingerprint.clone(),
            ssh_public_key: self.ssh_public_key.clone(),
            created_at: cursor::ts(self.created_at),
            updated_at: cursor::ts(self.updated_at),
            last_test_at: self.last_test_at.map(cursor::ts),
            last_test_ok: self.last_test_ok,
            last_test_error: self.last_test_error.clone(),
        }
    }

    /// Opens the stored secret (webhook secret or SSH key seed).
    pub async fn open_secret(
        &self,
        keyring: Option<&Keyring>,
    ) -> Result<Zeroizing<Vec<u8>>, ErrorClass> {
        let (Some(ct), Some(key_id)) = (&self.secret_ct, self.secret_key_id) else {
            return Err(ErrorClass::MissingSecret);
        };
        let keyring = keyring.ok_or(ErrorClass::CryptoUnavailable)?;
        keyring
            .decrypt(self.org_id, &secret_aad(self.org_id, self.id), key_id, ct)
            .await
            .map(Zeroizing::new)
            .map_err(|e| match e {
                CryptoError::Unavailable | CryptoError::KeyUnwrap => ErrorClass::CryptoUnavailable,
                CryptoError::Database(_) => ErrorClass::DatabaseUnavailable,
                _ => ErrorClass::MissingSecret,
            })
    }
}

/// KEY-3 AAD of `integration_destinations.secret_ct`.
pub fn secret_aad(org: Uuid, id: Uuid) -> Aad {
    Aad::new("integration_destinations", "secret_ct", org, id)
}

/// INTG-2 fingerprint: the first 8 hex digits of SHA-256.
pub fn fingerprint(bytes: &[u8]) -> String {
    export_build::hex(&Sha256::digest(bytes)[..4])
}

/// The config as stored and audited: the webhook URL's query string is dropped from audit
/// copies by [`audit_config`]; this is the full value.
pub fn config_json(config: &IntegrationConfig) -> Value {
    serde_json::to_value(config).expect("plain data")
}

/// The config for an audit row (trap 2, trap 20): the URL without its query string.
pub fn audit_config(config: &IntegrationConfig) -> Value {
    let mut c = config.clone();
    c.url = c.url.as_deref().map(netguard::redact_url);
    config_json(&c)
}

fn bad(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

/// INTG-1: trimmed, 1..120 characters, no control characters.
pub fn validate_name(raw: &str, errors: &mut Vec<InvalidParam>) -> String {
    let name = raw.trim().to_owned();
    let n = name.chars().count();
    if n == 0 || n > MAX_INTEGRATION_NAME_CHARS {
        errors.push(bad(
            "name",
            format!("must be 1..{MAX_INTEGRATION_NAME_CHARS} characters"),
        ));
    } else if name.chars().any(char::is_control) {
        errors.push(bad("name", "must not contain control characters"));
    }
    name
}

/// `Err(reason)` for kinds whose channel does not exist yet.
pub fn kind_available(kind: IntegrationKind) -> Result<(), &'static str> {
    match kind {
        IntegrationKind::Webhook | IntegrationKind::Sftp => Ok(()),
        IntegrationKind::FhirServer => {
            Err("fhir_server destinations are not available yet (WP-I.9)")
        }
        IntegrationKind::Hl7Mllp => Err("hl7_mllp destinations are not available yet (WP-I.11)"),
    }
}

fn safe_name_part(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// INTG-3: the keys `kind` needs, and nothing that belongs to another kind.
pub fn validate_config(kind: IntegrationKind, c: &IntegrationConfig) -> Vec<InvalidParam> {
    let mut e = Vec::new();
    let sftp_keys = c.host.is_some()
        || c.port.is_some()
        || c.username.is_some()
        || c.remote_dir.is_some()
        || c.host_key_sha256.is_some()
        || c.file_prefix.is_some();
    match kind {
        IntegrationKind::Webhook => {
            match c.url.as_deref() {
                None => e.push(bad("config.url", "is required for webhook destinations")),
                Some(url) => {
                    if let Err(why) = netguard::parse_webhook_url(url) {
                        e.push(bad("config.url", why));
                    }
                }
            }
            if sftp_keys {
                e.push(bad(
                    "config",
                    "sftp keys are not allowed on a webhook destination",
                ));
            }
        }
        IntegrationKind::Sftp => {
            if c.url.is_some() || !c.events.is_empty() {
                e.push(bad("config", "url and events are webhook keys"));
            }
            match c.host.as_deref().map(str::trim) {
                None | Some("") => e.push(bad("config.host", "is required for sftp destinations")),
                Some(h) => {
                    if h.len() > 253
                        || h.chars()
                            .any(|ch| ch.is_whitespace() || "/@?#".contains(ch))
                    {
                        e.push(bad("config.host", "must be a host name or address"));
                    }
                }
            }
            if c.port == Some(0) {
                e.push(bad("config.port", "must be 1..65535"));
            }
            match c.username.as_deref() {
                None | Some("") => e.push(bad("config.username", "is required")),
                Some(u) => {
                    if u.len() > 64 || !safe_name_part(u) {
                        e.push(bad("config.username", "must be 1..64 of [A-Za-z0-9._-]"));
                    }
                }
            }
            match c.remote_dir.as_deref() {
                None | Some("") => e.push(bad("config.remote_dir", "is required")),
                Some(d) => {
                    if !d.starts_with('/')
                        || d.len() > 512
                        || d.chars().any(char::is_control)
                        || d.split('/').any(|seg| seg == "..")
                    {
                        e.push(bad(
                            "config.remote_dir",
                            "must be an absolute path without `..` (at most 512 characters)",
                        ));
                    }
                }
            }
            match c.host_key_sha256.as_deref() {
                None => e.push(bad(
                    "config.host_key_sha256",
                    "is required: the server's host key is pinned (INTG-5)",
                )),
                Some(fp) => {
                    if ssh_keys::normalise_fingerprint(fp).is_none() {
                        e.push(bad(
                            "config.host_key_sha256",
                            "must be a SHA-256 fingerprint as printed by `ssh-keygen -lf` (SHA256:...)",
                        ));
                    }
                }
            }
            if let Some(p) = &c.file_prefix {
                if p.len() > 40 || !safe_name_part(p) {
                    e.push(bad("config.file_prefix", "must be 0..40 of [A-Za-z0-9._-]"));
                }
            }
        }
        IntegrationKind::FhirServer | IntegrationKind::Hl7Mllp => {}
    }
    if let Some(format) = c.format {
        if let Err(why) = export_build::format_available(format) {
            e.push(bad("config.format", why));
        }
    }
    if c.trigger == Some(DeliveryTrigger::OnFinalSign) {
        if c.format.is_none() {
            e.push(bad(
                "config.format",
                "is required with trigger on_final_sign (the auto-delivery format)",
            ));
        }
        if c.mode == Some(ExportMode::Deidentified) {
            e.push(bad(
                "config.mode",
                "auto-deliveries on final sign are identified clinical-care exports (DLV-6)",
            ));
        }
    }
    for (key, v) in [("config.hl7", &c.hl7), ("config.fhir", &c.fhir)] {
        if v.as_ref().is_some_and(|v| !v.is_object()) {
            e.push(bad(key, "must be an object"));
        }
    }
    e
}

/// DLV-2: `whsec_` + standard base64 of 24..64 bytes. Returns the HMAC key bytes. The
/// `Err` never repeats the input.
pub fn webhook_key(secret: &str) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let b64 = secret
        .strip_prefix(WEBHOOK_SECRET_PREFIX)
        .ok_or("must start with whsec_")?;
    let key = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|_| "must be whsec_ followed by standard base64")?,
    );
    if !(24..=64).contains(&key.len()) {
        return Err("must encode 24..64 random bytes");
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_secret_shape() {
        let good = format!(
            "whsec_{}",
            base64::engine::general_purpose::STANDARD.encode([7u8; 32])
        );
        assert_eq!(webhook_key(&good).unwrap().len(), 32);
        assert!(webhook_key("whsec_short").is_err());
        assert!(webhook_key("secret").is_err());
        assert_eq!(fingerprint(b"abc").len(), 8);
    }

    #[test]
    fn config_rules_per_kind() {
        let wh = IntegrationConfig {
            url: Some("https://his.example.org/hook".into()),
            ..Default::default()
        };
        assert!(validate_config(IntegrationKind::Webhook, &wh).is_empty());
        let mixed = IntegrationConfig {
            host: Some("sftp.example.org".into()),
            ..wh.clone()
        };
        assert_eq!(validate_config(IntegrationKind::Webhook, &mixed).len(), 1);
        let sftp = IntegrationConfig {
            host: Some("sftp.example.org".into()),
            username: Some("curv".into()),
            remote_dir: Some("/in".into()),
            ..Default::default()
        };
        let errs = validate_config(IntegrationKind::Sftp, &sftp);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert_eq!(errs[0].name, "config.host_key_sha256");
        let auto = IntegrationConfig {
            trigger: Some(DeliveryTrigger::OnFinalSign),
            ..wh
        };
        assert_eq!(
            validate_config(IntegrationKind::Webhook, &auto)[0].name,
            "config.format"
        );
    }
}

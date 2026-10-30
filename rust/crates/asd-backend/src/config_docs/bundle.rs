//! owner: WP-B.4
//!
//! Signed config bundles (CFG-1, CON-6, 02 ADR-B6).
//!
//! - [`BundleSigner`]: the Ed25519 key from `ASD_BUNDLE_SIGNING_KEY` (PKCS#8 PEM or the
//!   base64 32-byte seed, `asd-cli bundle keygen`). It must differ from the JWT key
//!   (AUTH-2); configuration refuses equal keys.
//! - [`compose`]: the three `published` documents -> `ConfigBundle` -> signed with
//!   `asd_contract::signing::sign_bundle`, verified again with `verify_bundle` (the only
//!   verifier, CON-6) and stored exactly as served. Versions are `YYYY.MM.DD-N` (UTC).
//! - [`latest_for_device`]: the newest bundle of the device's org, else the newest global
//!   default, re-verified before it is served; never an unsigned or tampered row.
//! - ETag = `"<bundle_version>"` (bundles are immutable and versions unique).

use std::collections::BTreeMap;
use std::fmt;

use asd_contract::api_v1::{ConfigBundleSummary, ConfigDocumentKind};
use asd_contract::config::{ConfigBundle, ScoringProfile};
use asd_contract::signing::{self, SigningKey, VerifyingKey};
use asd_core::types::{NormativeBaselineSet, StimulusCatalog};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{conflict, kind_str, ConfigApiError, ConfigApiResult, ALL_KINDS};
use crate::audit::AuditEvent;
use crate::auth::extractors::{RequestMeta, UserPrincipal};
use crate::auth::AuthConfig;
use crate::config::ConfigError;
use crate::error::AppError;
use crate::read::cursor::{self, Cursor};

/// Serialises bundle composition so `YYYY.MM.DD-N` numbering never races.
const COMPOSE_LOCK: i64 = 0x4153_4443_4647_0002;

/// The bundle signing key and its id. `Debug` prints the key id only.
#[derive(Clone, PartialEq, Eq)]
pub struct BundleSigner {
    key: SigningKey,
    key_id: String,
}

impl BundleSigner {
    pub fn new(key: SigningKey) -> Self {
        let key_id = signing::key_id(&key.verifying_key());
        Self { key, key_id }
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.key.verifying_key()
    }

    /// Reads `ASD_BUNDLE_SIGNING_KEY` (unset = no signing: `POST /v1/config/bundles`
    /// answers 503, existing bundles are still served). Refuses the JWT key (AUTH-2).
    pub fn from_lookup<G>(get: &G, auth: &AuthConfig) -> Result<Option<Self>, ConfigError>
    where
        G: Fn(&str) -> Option<String>,
    {
        const NAME: &str = "ASD_BUNDLE_SIGNING_KEY";
        let Some(raw) = get(NAME) else {
            return Ok(None);
        };
        let key = signing::parse_signing_key(&raw).map_err(|e| ConfigError::Invalid {
            name: NAME,
            reason: e.to_string(),
        })?;
        if let Some(jwt) = &auth.jwt {
            if key.verifying_key().to_bytes() == jwt.public_key_bytes() {
                return Err(ConfigError::Invalid {
                    name: NAME,
                    reason: "must differ from ASD_JWT_SIGNING_KEY".into(),
                });
            }
        }
        Ok(Some(Self::new(key)))
    }

    /// Signs `bundle` (its `key_id` is set to this key's id first).
    pub fn sign(&self, bundle: &mut ConfigBundle) -> Result<(), signing::SigningError> {
        bundle.key_id = self.key_id.clone();
        signing::sign_bundle(bundle, &self.key)
    }
}

impl fmt::Debug for BundleSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BundleSigner")
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

// ---- ETag / If-None-Match (pure) --------------------------------------------------------

/// The strong ETag of a bundle version.
pub fn etag(bundle_version: &str) -> String {
    format!("\"{bundle_version}\"")
}

/// RFC 9110 §13.1.2 weak comparison: `*`, or any listed tag (with or without `W/`) equal
/// to `etag`.
pub fn if_none_match_hits(header: &str, etag: &str) -> bool {
    let bare = |t: &str| t.trim().trim_start_matches("W/").to_owned();
    let target = bare(etag);
    header.split(',').any(|t| {
        let t = t.trim();
        t == "*" || bare(t) == target
    })
}

/// `YYYY.MM.DD-N` for the `n`-th bundle of `day` (1-based).
pub fn bundle_version(day: DateTime<Utc>, n: i64) -> String {
    format!("{}-{n}", day.format("%Y.%m.%d"))
}

// ---- composition ------------------------------------------------------------------------

struct Published {
    id: Uuid,
    version: String,
    body: Value,
}

async fn published(
    conn: &mut PgConnection,
    kind: ConfigDocumentKind,
) -> ConfigApiResult<Option<Published>> {
    Ok(sqlx::query_as!(
        Published,
        "SELECT id, version, body FROM config_documents
         WHERE kind = $1 AND status = 'published'
         ORDER BY published_at DESC LIMIT 1",
        kind_str(kind)
    )
    .fetch_optional(&mut *conn)
    .await?)
}

fn decode<T: serde::de::DeserializeOwned>(doc: &Published, kind: &str) -> ConfigApiResult<T> {
    serde_json::from_value(doc.body.clone()).map_err(|e| {
        conflict(
            "config-invalid",
            format!("published {kind} `{}` does not decode: {e}", doc.version),
        )
    })
}

/// Composes, signs and stores a bundle for `org` (`None` = the global default).
pub async fn compose(
    conn: &mut PgConnection,
    signer: Option<&BundleSigner>,
    actor: &UserPrincipal,
    meta: &RequestMeta,
    org: Option<Uuid>,
    feature_flags: BTreeMap<String, bool>,
) -> ConfigApiResult<ConfigBundleSummary> {
    let signer = signer.ok_or(ConfigApiError::SigningUnavailable)?;
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", COMPOSE_LOCK)
        .execute(&mut *conn)
        .await?;

    let mut docs = Vec::new();
    let mut missing = Vec::new();
    for kind in ALL_KINDS {
        match published(conn, kind).await? {
            Some(d) => docs.push(d),
            None => missing.push(kind_str(kind)),
        }
    }
    if !missing.is_empty() {
        return Err(conflict(
            "config-not-published",
            format!("no published document of kind {}", missing.join(", ")),
        ));
    }
    let [catalog_doc, baselines_doc, scoring_doc]: [Published; 3] = docs
        .try_into()
        .map_err(|_| AppError::Internal("three kinds".into()))?;
    let catalog: StimulusCatalog = decode(&catalog_doc, "stimulus_catalog")?;
    let baselines: NormativeBaselineSet = decode(&baselines_doc, "normative_baselines")?;
    let scoring_profile: ScoringProfile = decode(&scoring_doc, "scoring_profile")?;

    let now = Utc::now();
    let prefix = format!("{}-", now.format("%Y.%m.%d"));
    let today = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM config_bundles WHERE starts_with(bundle_version, $1)"#,
        prefix
    )
    .fetch_one(&mut *conn)
    .await?;
    let version = bundle_version(now, today + 1);

    let mut bundle = ConfigBundle {
        bundle_version: version.clone(),
        key_id: String::new(),
        catalog,
        baselines,
        scoring_profile,
        feature_flags,
        signed_at: cursor::ts(now),
        signature: String::new(),
    };
    signer
        .sign(&mut bundle)
        .map_err(|e| AppError::Internal(format!("signing failed: {e}")))?;
    let body =
        serde_json::to_value(&bundle).map_err(|e| AppError::Internal(format!("encode: {e}")))?;
    // Never store what the device-side verifier would reject.
    signing::verify_bundle(&body.to_string(), &signer.verifying_key())
        .map_err(|e| AppError::Internal(format!("self-verification failed: {e}")))?;

    let row = sqlx::query!(
        "INSERT INTO config_bundles
           (org_id, catalog_id, baselines_id, scoring_id, bundle_version, body, signature, key_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, published_at",
        org,
        catalog_doc.id,
        baselines_doc.id,
        scoring_doc.id,
        version,
        body,
        bundle.signature,
        bundle.key_id
    )
    .fetch_one(&mut *conn)
    .await?;

    let summary = ConfigBundleSummary {
        id: row.id.to_string(),
        bundle_version: version,
        catalog_version: catalog_doc.version,
        baselines_version: baselines_doc.version,
        scoring_profile_version: scoring_doc.version,
        key_id: bundle.key_id,
        published_at: cursor::ts(row.published_at),
    };
    let mut event = AuditEvent::user(actor, "config.bundle.publish", "config_bundle", row.id)
        .after(json!({
            "bundle_version": summary.bundle_version,
            "org_id": org,
            "catalog_version": summary.catalog_version,
            "baselines_version": summary.baselines_version,
            "scoring_profile_version": summary.scoring_profile_version,
            "key_id": summary.key_id,
        }))
        .meta(meta);
    if let Some(org) = org {
        event = event.org(org);
    }
    event.record(conn).await?;
    Ok(summary)
}

// ---- reads ------------------------------------------------------------------------------

struct SummaryRow {
    id: Uuid,
    bundle_version: String,
    key_id: Option<String>,
    published_at: DateTime<Utc>,
    catalog_version: String,
    baselines_version: String,
    scoring_profile_version: String,
}

impl SummaryRow {
    fn into_dto(self) -> ConfigBundleSummary {
        ConfigBundleSummary {
            id: self.id.to_string(),
            bundle_version: self.bundle_version,
            catalog_version: self.catalog_version,
            baselines_version: self.baselines_version,
            scoring_profile_version: self.scoring_profile_version,
            key_id: self.key_id.unwrap_or_default(),
            published_at: cursor::ts(self.published_at),
        }
    }
}

/// Bundles visible to `viewer`, newest first: its org's and the global ones (all of them
/// for a superadmin).
pub async fn list(
    conn: &mut PgConnection,
    viewer: &UserPrincipal,
    after: Option<&Cursor>,
    limit: i64,
) -> ConfigApiResult<(Vec<ConfigBundleSummary>, Option<String>)> {
    let (after_at, after_id) = match after {
        Some(c) => (
            Some(c.at),
            Some(Uuid::parse_str(&c.id).map_err(|_| {
                AppError::Validation(vec![crate::problem::InvalidParam {
                    name: "cursor".into(),
                    reason: "not a cursor from this API".into(),
                }])
            })?),
        ),
        None => (None, None),
    };
    let mut rows = sqlx::query_as!(
        SummaryRow,
        "SELECT b.id, b.bundle_version, b.key_id, b.published_at,
                c.version AS catalog_version, n.version AS baselines_version,
                s.version AS scoring_profile_version
         FROM config_bundles b
         JOIN config_documents c ON c.id = b.catalog_id
         JOIN config_documents n ON n.id = b.baselines_id
         JOIN config_documents s ON s.id = b.scoring_id
         WHERE ($1 OR b.org_id IS NULL OR b.org_id = $2)
           AND ($3::timestamptz IS NULL OR (b.published_at, b.id) < ($3, $4))
         ORDER BY b.published_at DESC, b.id DESC
         LIMIT $5",
        viewer.is_superadmin(),
        viewer.org_id,
        after_at,
        after_id,
        limit + 1
    )
    .fetch_all(&mut *conn)
    .await?;
    let next = if rows.len() > usize::try_from(limit).unwrap_or(usize::MAX) {
        rows.pop();
        rows.last()
            .map(|r| Cursor::new(r.published_at, r.id.to_string()).encode())
    } else {
        None
    };
    Ok((rows.into_iter().map(SummaryRow::into_dto).collect(), next))
}

/// A stored bundle, re-verified, as the bytes to serve.
#[derive(Debug, Clone)]
pub struct Served {
    pub bundle_version: String,
    pub json: Vec<u8>,
}

/// Checks a stored body before it leaves the server: it must carry an `ed25519:`
/// signature and, when signed with the current key, verify (CON-6). Bundles signed with a
/// previous key (rotation) are served as stored; devices trust both keys by `key_id`.
fn checked(
    bundle_version: String,
    body: &Value,
    signer: Option<&BundleSigner>,
) -> ConfigApiResult<Served> {
    let fail = |why: String| {
        tracing::error!(bundle_version = %bundle_version, reason = %why, "stored bundle refused");
        ConfigApiError::App(AppError::Internal(format!("stored bundle refused: {why}")))
    };
    let json = serde_json::to_string(body).map_err(|e| fail(e.to_string()))?;
    let signed = body
        .get(signing::SIGNATURE_FIELD)
        .and_then(Value::as_str)
        .is_some_and(|s| s.starts_with(signing::SIGNATURE_PREFIX));
    if !signed {
        return Err(fail("no signature".into()));
    }
    if body.get("bundle_version").and_then(Value::as_str) != Some(bundle_version.as_str()) {
        return Err(fail("bundle_version does not match its row".into()));
    }
    match signer {
        Some(s) if body.get("key_id").and_then(Value::as_str) == Some(s.key_id()) => {
            signing::verify_bundle(&json, &s.verifying_key()).map_err(|e| fail(e.to_string()))?;
        }
        _ => {
            serde_json::from_value::<ConfigBundle>(body.clone())
                .map_err(|e| fail(e.to_string()))?;
        }
    }
    Ok(Served {
        bundle_version,
        json: json.into_bytes(),
    })
}

/// The newest bundle for `org`, else the newest global default; `None` when there is
/// none. Same rule as `config_bundle_version` in `POST /v1/devices/register`.
pub async fn latest_for_org(
    conn: &mut PgConnection,
    signer: Option<&BundleSigner>,
    org: Uuid,
) -> ConfigApiResult<Option<Served>> {
    let row = sqlx::query!(
        "SELECT bundle_version, body FROM config_bundles
         WHERE org_id = $1 OR org_id IS NULL
         ORDER BY (org_id IS NULL), published_at DESC
         LIMIT 1",
        org
    )
    .fetch_optional(&mut *conn)
    .await?;
    row.map(|r| checked(r.bundle_version, &r.body, signer))
        .transpose()
}

/// One bundle by version, if `viewer` may see it (its org's, global, or superadmin).
pub async fn by_version(
    conn: &mut PgConnection,
    signer: Option<&BundleSigner>,
    viewer: &UserPrincipal,
    version: &str,
) -> ConfigApiResult<Served> {
    let row = sqlx::query!(
        "SELECT bundle_version, body, org_id FROM config_bundles WHERE bundle_version = $1",
        version
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    if let Some(org) = row.org_id {
        viewer.ensure_org(org)?;
    }
    checked(row.bundle_version, &row.body, signer)
}

/// Records that `device` has `bundle_version` (first time only; adoption metrics).
pub async fn record_adoption(
    conn: &mut PgConnection,
    device: Uuid,
    bundle_version: &str,
) -> ConfigApiResult<()> {
    sqlx::query!(
        "INSERT INTO bundle_adoption (device_id, bundle_version) VALUES ($1, $2)
         ON CONFLICT (device_id, bundle_version) DO NOTHING",
        device,
        bundle_version
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::collections::HashMap;

    fn seed_b64(seed: [u8; 32]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(seed)
    }

    fn cfg(pairs: &[(&str, String)]) -> Result<Config, ConfigError> {
        let mut map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        map.insert("DATABASE_URL".into(), "postgres://a@b/c".into());
        Config::from_lookup(|k| map.get(k).cloned())
    }

    #[test]
    fn signing_key_is_optional_parsed_and_never_the_jwt_key() {
        assert!(cfg(&[]).unwrap().bundle_signing.is_none());

        let c = cfg(&[("ASD_BUNDLE_SIGNING_KEY", seed_b64([1; 32]))]).unwrap();
        let signer = c.bundle_signing.expect("configured");
        let expected = signing::signing_key_from_seed([1; 32]);
        assert_eq!(signer.key_id(), signing::key_id(&expected.verifying_key()));

        // PEM works too (asd-cli bundle keygen output).
        let pem = signing::signing_key_to_pkcs8_pem(&expected).unwrap();
        let c = cfg(&[("ASD_BUNDLE_SIGNING_KEY", pem)]).unwrap();
        assert_eq!(c.bundle_signing.unwrap(), signer);

        // Different JWT and bundle keys: fine.
        let c = cfg(&[
            ("ASD_BUNDLE_SIGNING_KEY", seed_b64([1; 32])),
            ("ASD_JWT_SIGNING_KEY", seed_b64([2; 32])),
        ]);
        assert!(c.is_ok(), "{c:?}");

        // The same key for both: refused (AUTH-2).
        let err = cfg(&[
            ("ASD_BUNDLE_SIGNING_KEY", seed_b64([3; 32])),
            ("ASD_JWT_SIGNING_KEY", seed_b64([3; 32])),
        ])
        .unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }), "{err:?}");

        let err = cfg(&[("ASD_BUNDLE_SIGNING_KEY", "not a key".into())]).unwrap_err();
        assert!(
            matches!(
                err,
                ConfigError::Invalid {
                    name: "ASD_BUNDLE_SIGNING_KEY",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn debug_never_prints_the_key() {
        let key = signing::signing_key_from_seed([9; 32]);
        let pem = signing::signing_key_to_pkcs8_pem(&key).unwrap();
        let c = cfg(&[("ASD_BUNDLE_SIGNING_KEY", seed_b64([9; 32]))]).unwrap();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("BundleSigner"), "{dbg}");
        assert!(!dbg.contains(&seed_b64([9; 32])), "{dbg}");
        let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
        assert!(!dbg.contains(&body), "{dbg}");
    }

    #[test]
    fn etags_and_if_none_match() {
        let tag = etag("2026.10.30-1");
        assert_eq!(tag, "\"2026.10.30-1\"");
        assert!(if_none_match_hits(&tag, &tag));
        assert!(if_none_match_hits("W/\"2026.10.30-1\"", &tag));
        assert!(if_none_match_hits("\"a\", \"2026.10.30-1\"", &tag));
        assert!(if_none_match_hits(" * ", &tag));
        assert!(!if_none_match_hits("\"2026.10.30-2\"", &tag));
        assert!(!if_none_match_hits("2026.10.30-1x", &tag));
        assert!(!if_none_match_hits("", &tag));
    }

    #[test]
    fn versions_are_dated_and_numbered() {
        let day = DateTime::parse_from_rfc3339("2026-01-05T23:59:59Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(bundle_version(day, 1), "2026.01.05-1");
        assert_eq!(bundle_version(day, 12), "2026.01.05-12");
    }

    #[test]
    fn stored_bodies_are_checked_before_serving() {
        let signer = BundleSigner::new(signing::signing_key_from_seed([4; 32]));
        let mut bundle = ConfigBundle {
            bundle_version: "v1".into(),
            key_id: String::new(),
            catalog: serde_json::from_str(include_str!("../../seed/stimulus_catalog.json"))
                .unwrap(),
            baselines: serde_json::from_str(include_str!("../../seed/normative_baselines.json"))
                .unwrap(),
            scoring_profile: ScoringProfile::default(),
            feature_flags: BTreeMap::new(),
            signed_at: "2026-10-30T00:00:00.000Z".into(),
            signature: String::new(),
        };
        signer.sign(&mut bundle).unwrap();
        let body = serde_json::to_value(&bundle).unwrap();
        let served = checked("v1".into(), &body, Some(&signer)).unwrap();
        signing::verify_bundle(
            std::str::from_utf8(&served.json).unwrap(),
            &signer.verifying_key(),
        )
        .unwrap();

        assert!(
            checked("v2".into(), &body, Some(&signer)).is_err(),
            "row mismatch"
        );
        let mut tampered = body.clone();
        tampered["scoring_profile"]["flag"]["greenFloor"] = json!(0.1);
        assert!(checked("v1".into(), &tampered, Some(&signer)).is_err());
        let mut unsigned = body.clone();
        unsigned.as_object_mut().unwrap().remove("signature");
        assert!(checked("v1".into(), &unsigned, Some(&signer)).is_err());
        assert!(checked("v1".into(), &unsigned, None).is_err());
        // Signed by a rotated-out key: served as stored.
        let other = BundleSigner::new(signing::signing_key_from_seed([5; 32]));
        assert!(checked("v1".into(), &body, Some(&other)).is_ok());
    }
}

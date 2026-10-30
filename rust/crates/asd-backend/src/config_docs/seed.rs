//! owner: WP-B.4
//!
//! CFG-2: on first boot with no config documents, seed one `draft` of each kind from the
//! kit's bundled JSON (`domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources/`,
//! copied byte-for-byte into `asd-backend/seed/` because the Docker build context is
//! `rust/`; a test fails when the copies drift) plus the default `ScoringProfile` (equal to
//! the Swift constants, CON-5). Seeded documents have no author (`created_by NULL`); the
//! four-eyes rule then needs two different people to approve and publish them. Nothing is
//! ever published here.

use asd_contract::api_v1::ConfigDocumentKind;
use asd_contract::config::ScoringProfile;
use serde_json::{json, Value};
use sqlx::PgPool;

use super::kind_str;
use crate::audit::AuditEvent;
use crate::error::{AppError, AppResult};

/// The kit's `stimulus_catalog.json`.
pub const STIMULUS_CATALOG_JSON: &str = include_str!("../../seed/stimulus_catalog.json");
/// The kit's `normative_baselines.json`.
pub const NORMATIVE_BASELINES_JSON: &str = include_str!("../../seed/normative_baselines.json");

/// Serialises concurrent first boots (two API replicas).
const SEED_LOCK: i64 = 0x4153_4443_4647_0003;

/// The documents seeded on first boot: `(kind, version, body)`. Versions are
/// `bundled-<schemaVersion>`.
pub fn seed_documents() -> AppResult<Vec<(ConfigDocumentKind, String, Value)>> {
    let parse = |raw: &str, what: &str| -> AppResult<Value> {
        serde_json::from_str(raw).map_err(|e| AppError::Internal(format!("seed {what}: {e}")))
    };
    let profile = serde_json::to_value(ScoringProfile::default())
        .map_err(|e| AppError::Internal(format!("seed scoring profile: {e}")))?;
    let docs = [
        (
            ConfigDocumentKind::StimulusCatalog,
            parse(STIMULUS_CATALOG_JSON, "catalog")?,
        ),
        (
            ConfigDocumentKind::NormativeBaselines,
            parse(NORMATIVE_BASELINES_JSON, "baselines")?,
        ),
        (ConfigDocumentKind::ScoringProfile, profile),
    ];
    Ok(docs
        .into_iter()
        .map(|(kind, body)| {
            let schema = body["schemaVersion"].as_str().unwrap_or("0").to_owned();
            (kind, format!("bundled-{schema}"), body)
        })
        .collect())
}

/// Seeds the drafts when `config_documents` is empty; returns how many were inserted
/// (0 when any document already exists). One `config.document.seed` audit row.
pub async fn seed_if_empty(pool: &PgPool) -> AppResult<usize> {
    let docs = seed_documents()?;
    let mut tx = pool.begin().await?;
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", SEED_LOCK)
        .execute(&mut *tx)
        .await?;
    let any = sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM config_documents) AS "any!""#)
        .fetch_one(&mut *tx)
        .await?;
    if any {
        return Ok(0);
    }
    let mut seeded = Vec::new();
    for (kind, version, body) in &docs {
        let id = sqlx::query_scalar!(
            "INSERT INTO config_documents (kind, version, status, body, notes)
             VALUES ($1, $2, 'draft', $3, 'Seeded on first boot from the app''s bundled defaults (CFG-2)')
             ON CONFLICT (kind, version) DO NOTHING
             RETURNING id",
            kind_str(*kind),
            version,
            body
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(id) = id {
            seeded.push(json!({"id": id, "kind": kind_str(*kind), "version": version}));
        }
    }
    let n = seeded.len();
    AuditEvent::system("config.document.seed", "config_document", "seed")
        .after(json!({"status": "draft", "documents": seeded}))
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    tracing::info!(documents = n, "seeded draft config documents (CFG-2)");
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn seed_copies_equal_the_kit_resources() {
        let kit = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources");
        for (name, embedded) in [
            ("stimulus_catalog.json", STIMULUS_CATALOG_JSON),
            ("normative_baselines.json", NORMATIVE_BASELINES_JSON),
        ] {
            let kit_file = std::fs::read_to_string(kit.join(name))
                .unwrap_or_else(|e| panic!("read kit {name}: {e}"));
            assert_eq!(
                embedded, kit_file,
                "asd-backend/seed/{name} drifted from the kit; copy it again"
            );
        }
    }

    #[test]
    fn seed_documents_are_valid_drafts_of_each_kind() {
        let docs = seed_documents().unwrap();
        assert_eq!(docs.len(), 3);
        for (kind, version, body) in &docs {
            assert!(version.starts_with("bundled-"), "{version}");
            super::super::lifecycle::validate_version(version).unwrap();
            super::super::lifecycle::validate_body(*kind, body)
                .unwrap_or_else(|e| panic!("{kind:?}: {e:?}"));
        }
        assert_eq!(
            docs[2].2,
            serde_json::to_value(ScoringProfile::default()).unwrap()
        );
    }
}

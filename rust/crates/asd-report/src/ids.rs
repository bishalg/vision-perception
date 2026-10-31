//! Deterministic ids and timestamps shared by the builders.

use chrono::{DateTime, SecondsFormat, Utc};
use uuid::Uuid;

use crate::ReportError;

/// Base of every local FHIR URI (`FHIRNamespaces.local`, ADR-0003 decision 7).
pub(crate) const LOCAL: &str = "http://curv.org/fhir";

/// RFC 4122 v5 (SHA-1) UUID in the URL namespace for `<LOCAL>/<name>`, lower-case
/// (`FHIRBundleBuilder.uuid`).
pub(crate) fn uuid5(name: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("{LOCAL}/{name}").as_bytes()).to_string()
}

pub(crate) fn urn(id: &str) -> String {
    format!("urn:uuid:{id}")
}

pub(crate) fn parse(field: &'static str, rfc3339: &str) -> Result<DateTime<Utc>, ReportError> {
    DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| ReportError::InvalidTimestamp { field })
}

/// ISO 8601 in UTC with whole seconds (`2026-10-01T10:00:00Z`), as Foundation's
/// `ISO8601DateFormatter` writes it: fractional seconds are dropped, not rounded.
pub(crate) fn fhir_timestamp(field: &'static str, rfc3339: &str) -> Result<String, ReportError> {
    let t = parse(field, rfc3339)?;
    let whole = DateTime::<Utc>::from_timestamp(t.timestamp(), 0)
        .ok_or(ReportError::InvalidTimestamp { field })?;
    Ok(whole.to_rfc3339_opts(SecondsFormat::Secs, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid5_matches_the_ios_golden_patient_url() {
        // expected-bundle.json: Patient fullUrl for token child-GOLDEN01.
        assert_eq!(
            uuid5("Patient/child-GOLDEN01"),
            "a5ace1c8-3fcd-5498-a817-fe0765919ea7"
        );
    }

    #[test]
    fn timestamps_truncate_to_whole_seconds_in_utc() {
        assert_eq!(
            fhir_timestamp("t", "2026-10-26T07:33:52.999Z").unwrap(),
            "2026-10-26T07:33:52Z"
        );
        assert_eq!(
            fhir_timestamp("t", "2026-10-26T09:33:52+02:00").unwrap(),
            "2026-10-26T07:33:52Z"
        );
        assert!(fhir_timestamp("t", "2026-10-26").is_err());
    }
}

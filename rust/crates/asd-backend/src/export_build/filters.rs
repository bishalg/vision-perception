//! owner: WP-I.6
//!
//! EXJ-1 filters: the session-list filters (parsed by the same code as
//! `GET /v1/orgs/{id}/sessions`, so `from=2026-10-01` means the same thing in both places)
//! plus participant ids, session ids and clinical review states. Validated at request time
//! (`400 validation-failed`) and parsed again by the job from the stored copy.

use asd_contract::clinical::ClinicalReviewState;
use asd_contract::export::{ExportFilters, MAX_FILTER_IDS};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::sessions::SessionQuery;

/// `clinical_state` value selecting sessions without a clinical review.
pub const NO_CLINICAL_REVIEW: &str = "none";

/// Validated filters, ready for SQL.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedFilters {
    pub flag: Option<String>,
    pub quality: Option<String>,
    pub from: Option<DateTime<Utc>>,
    /// Inclusive upper bound.
    pub to: Option<DateTime<Utc>>,
    pub bundle_version: Option<String>,
    pub reviewed: Option<bool>,
    pub participant_ids: Vec<Uuid>,
    pub session_ids: Vec<String>,
    /// Review states, `none` included as is.
    pub clinical_state: Vec<String>,
}

fn bad(name: &str, reason: &str) -> InvalidParam {
    InvalidParam {
        name: format!("filters.{name}"),
        reason: reason.into(),
    }
}

/// Validates `f`; every problem is reported at once (`filters.<field>`).
pub fn parse(f: &ExportFilters) -> AppResult<ParsedFilters> {
    let mut errors = Vec::new();
    let session = SessionQuery {
        flag: f.flag.clone(),
        quality: f.quality.clone(),
        from: f.from.clone(),
        to: f.to.clone(),
        bundle_version: f.bundle_version.clone(),
        reviewed: None,
        tracking: None,
        cursor: None,
        limit: None,
    }
    .parse();
    let session = match session {
        Ok(s) => Some(s),
        Err(AppError::Validation(params)) => {
            errors.extend(params.into_iter().map(|p| bad(&p.name, &p.reason)));
            None
        }
        Err(other) => return Err(other),
    };

    for (name, len) in [
        ("participant_ids", f.participant_ids.len()),
        ("session_ids", f.session_ids.len()),
        ("clinical_state", f.clinical_state.len()),
    ] {
        if len > MAX_FILTER_IDS {
            errors.push(bad(name, &format!("at most {MAX_FILTER_IDS} values")));
        }
    }
    let mut participant_ids = Vec::with_capacity(f.participant_ids.len());
    for raw in &f.participant_ids {
        match Uuid::parse_str(raw) {
            Ok(id) => participant_ids.push(id),
            Err(_) => {
                errors.push(bad("participant_ids", "must be UUIDs"));
                break;
            }
        }
    }
    if f.session_ids
        .iter()
        .any(|s| s.is_empty() || s.len() > 128 || s.chars().any(char::is_control))
    {
        errors.push(bad("session_ids", "must be session ids"));
    }
    if f.clinical_state
        .iter()
        .any(|s| s != NO_CLINICAL_REVIEW && ClinicalReviewState::parse(s).is_none())
    {
        errors.push(bad(
            "clinical_state",
            "must be open, in_progress, awaiting_cosign, final, amending or none",
        ));
    }

    match session {
        Some(s) if errors.is_empty() => Ok(ParsedFilters {
            flag: s.flag,
            quality: s.quality,
            from: s.from,
            to: s.to,
            bundle_version: s.bundle_version,
            reviewed: f.reviewed,
            participant_ids,
            session_ids: f.session_ids.clone(),
            clinical_state: f.clinical_state.clone(),
        }),
        _ => Err(AppError::Validation(errors)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_filters_select_everything() {
        let p = parse(&ExportFilters::default()).unwrap();
        assert_eq!(p, ParsedFilters::default());
    }

    #[test]
    fn dates_ids_and_states_are_parsed() {
        let id = Uuid::new_v4();
        let p = parse(&ExportFilters {
            flag: Some("red".into()),
            from: Some("2026-10-01".into()),
            to: Some("2026-10-02".into()),
            reviewed: Some(true),
            participant_ids: vec![id.to_string()],
            session_ids: vec!["S-1".into()],
            clinical_state: vec!["final".into(), "none".into()],
            ..ExportFilters::default()
        })
        .unwrap();
        assert_eq!(p.flag.as_deref(), Some("red"));
        assert_eq!(p.from.unwrap().to_rfc3339(), "2026-10-01T00:00:00+00:00");
        assert_eq!(p.participant_ids, vec![id]);
        assert_eq!(p.reviewed, Some(true));
    }

    #[test]
    fn every_problem_is_reported_under_filters() {
        let Err(AppError::Validation(p)) = parse(&ExportFilters {
            flag: Some("purple".into()),
            from: Some("last week".into()),
            participant_ids: vec!["nope".into()],
            clinical_state: vec!["signed".into()],
            ..ExportFilters::default()
        }) else {
            panic!("expected validation errors")
        };
        let names: Vec<_> = p.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "filters.flag",
                "filters.from",
                "filters.participant_ids",
                "filters.clinical_state"
            ]
        );
    }
}

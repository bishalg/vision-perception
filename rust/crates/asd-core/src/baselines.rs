//! owner: WP-R.1
//!
//! Baseline lookups and coverage validation: port of the logic in
//! `NormativeBaselineLibrary.swift` (`bracketId(forAgeMonths:)`, `range(forBracketId:)`,
//! `NormativeBaselineLibrary.validate`). CORE-3.

use crate::types::{NormativeBaselineSet, NormativeExpectation, NormativeRange};

/// Key of the fallback range in `NormativeExpectation::by_age`.
pub const DEFAULT_BRACKET_ID: &str = "default";

/// Id of the first declared bracket containing `age_months` (bounds inclusive); `None`
/// when the age is outside every bracket. Same as `NormativeBaselineSet.bracketId(forAgeMonths:)`.
pub fn bracket_id_for(set: &NormativeBaselineSet, age_months: i64) -> Option<&str> {
    set.age_brackets
        .iter()
        .find(|b| age_months >= b.min_months && age_months <= b.max_months)
        .map(|b| b.id.as_str())
}

/// Range for a bracket id, falling back to `"default"`. Same as
/// `NormativeExpectation.range(forBracketId:)`.
pub fn range_for<'a>(
    expectation: &'a NormativeExpectation,
    bracket_id: &str,
) -> Option<&'a NormativeRange> {
    expectation
        .by_age
        .get(bracket_id)
        .or_else(|| expectation.by_age.get(DEFAULT_BRACKET_ID))
}

/// Coverage problems of a baseline set, as human-readable lines with the Swift text.
///
/// Order: one `missing video profile` line per required id (in the given order), then per
/// video in ascending id order. Swift iterates a `Dictionary` here, so its order is
/// unspecified; parity tests compare the Swift output after the same ordering.
pub fn validate<S: AsRef<str>>(
    set: &NormativeBaselineSet,
    required_video_ids: &[S],
) -> Vec<String> {
    let mut problems = Vec::new();
    for id in required_video_ids {
        let id = id.as_ref();
        if !set.videos.contains_key(id) {
            problems.push(format!("missing video profile: {id}"));
        }
    }
    for (video_id, video) in &set.videos {
        if video.segments.is_empty() {
            problems.push(format!("{video_id}: no segments"));
        }
        for segment in &video.segments {
            for expectation in &segment.expectations {
                for bracket in &set.age_brackets {
                    if range_for(expectation, &bracket.id).is_none() {
                        problems.push(format!(
                            "{video_id}/{}/{}: no range for bracket {}",
                            segment.id, expectation.biomarker, bracket.id
                        ));
                    }
                }
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgeBracket, NormativeDirection};
    use std::collections::BTreeMap;

    fn expectation(keys: &[&str]) -> NormativeExpectation {
        NormativeExpectation {
            biomarker: "b".into(),
            direction: NormativeDirection::HigherBetter,
            target_aoi: None,
            by_age: keys
                .iter()
                .map(|k| {
                    (
                        (*k).to_owned(),
                        NormativeRange {
                            expected: Some(k.len() as f64),
                            acceptable_low: None,
                            acceptable_high: None,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            citation: "c".into(),
            source: "literature".into(),
            note: None,
        }
    }

    #[test]
    fn bracket_range_wins_over_default() {
        let e = expectation(&["default", "12-17x"]);
        assert_eq!(range_for(&e, "12-17x").unwrap().expected, Some(6.0));
        assert_eq!(range_for(&e, "other").unwrap().expected, Some(7.0));
        assert!(range_for(&expectation(&["a"]), "b").is_none());
    }

    #[test]
    fn bracket_bounds_are_inclusive() {
        let set = NormativeBaselineSet {
            schema_version: "t".into(),
            status: "t".into(),
            provenance: None,
            age_brackets: vec![AgeBracket {
                id: "x".into(),
                min_months: 10,
                max_months: 12,
            }],
            videos: BTreeMap::new(),
        };
        assert_eq!(bracket_id_for(&set, 10), Some("x"));
        assert_eq!(bracket_id_for(&set, 12), Some("x"));
        assert_eq!(bracket_id_for(&set, 13), None);
    }
}

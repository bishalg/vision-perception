//! XC-5 consent eligibility: the full table of purpose x mode x consent facts.

use asd_export::policy::{
    eligibility, validate_request, ConsentFacts, Eligibility, ExclusionReason, Mode, PolicyError,
    Purpose,
};

use Eligibility::{Exclude, Include};
use ExclusionReason::{ModeNotAllowed, NoResearchConsent, Withdrawn};

const WITHDRAWN: ConsentFacts = ConsentFacts {
    withdrawn: true,
    research_consented: Some(true),
};
const CONSENTED: ConsentFacts = ConsentFacts {
    withdrawn: false,
    research_consented: Some(true),
};
const NOT_CONSENTED: ConsentFacts = ConsentFacts {
    withdrawn: false,
    research_consented: Some(false),
};
const CONSENT_UNKNOWN: ConsentFacts = ConsentFacts {
    withdrawn: false,
    research_consented: None,
};

#[test]
fn eligibility_table() {
    use Mode::{Deidentified as D, Identified as I};
    use Purpose::*;
    #[rustfmt::skip]
    let table: &[(Purpose, Mode, ConsentFacts, Eligibility)] = &[
        // Withdrawal excludes always, whatever the purpose or mode.
        (ClinicalCare,       I, WITHDRAWN,     Exclude(Withdrawn)),
        (ClinicalCare,       D, WITHDRAWN,     Exclude(Withdrawn)),
        (Research,           I, WITHDRAWN,     Exclude(Withdrawn)),
        (Research,           D, WITHDRAWN,     Exclude(Withdrawn)),
        (QualityImprovement, I, WITHDRAWN,     Exclude(Withdrawn)),
        (QualityImprovement, D, WITHDRAWN,     Exclude(Withdrawn)),
        (PatientRequest,     I, WITHDRAWN,     Exclude(Withdrawn)),
        (PatientRequest,     D, WITHDRAWN,     Exclude(Withdrawn)),
        // Clinical care and patient request: both modes, research consent irrelevant.
        (ClinicalCare,       I, CONSENTED,     Include),
        (ClinicalCare,       D, CONSENTED,     Include),
        (ClinicalCare,       I, NOT_CONSENTED, Include),
        (ClinicalCare,       D, NOT_CONSENTED, Include),
        (PatientRequest,     I, CONSENTED,     Include),
        (PatientRequest,     D, CONSENTED,     Include),
        (PatientRequest,     I, NOT_CONSENTED, Include),
        (PatientRequest,     D, NOT_CONSENTED, Include),
        // Research: de-identified only, and only sessions with normCollection.consented.
        (Research,           I, CONSENTED,     Exclude(ModeNotAllowed)),
        (Research,           D, CONSENTED,     Include),
        (Research,           I, NOT_CONSENTED, Exclude(ModeNotAllowed)),
        (Research,           D, NOT_CONSENTED, Exclude(NoResearchConsent)),
        // Quality improvement: de-identified only, research consent irrelevant.
        (QualityImprovement, I, CONSENTED,     Exclude(ModeNotAllowed)),
        (QualityImprovement, D, CONSENTED,     Include),
        (QualityImprovement, I, NOT_CONSENTED, Exclude(ModeNotAllowed)),
        (QualityImprovement, D, NOT_CONSENTED, Include),
    ];
    assert_eq!(table.len(), 4 * 2 * 3);
    for (purpose, mode, facts, expected) in table {
        assert_eq!(
            eligibility(*purpose, *mode, facts),
            *expected,
            "{purpose:?} {mode:?} {facts:?}"
        );
    }
}

#[test]
fn research_without_a_recorded_consent_is_excluded() {
    // G15: an absent `normCollection.consented` is not consent.
    assert_eq!(
        eligibility(Purpose::Research, Mode::Deidentified, &CONSENT_UNKNOWN),
        Exclude(NoResearchConsent)
    );
    assert_eq!(
        eligibility(Purpose::ClinicalCare, Mode::Identified, &CONSENT_UNKNOWN),
        Include
    );
}

#[test]
fn request_validation_rejects_identified_research_and_quality_improvement() {
    for purpose in [Purpose::ClinicalCare, Purpose::PatientRequest] {
        for mode in [Mode::Identified, Mode::Deidentified] {
            assert_eq!(validate_request(purpose, mode), Ok(()));
        }
    }
    for purpose in [Purpose::Research, Purpose::QualityImprovement] {
        assert_eq!(validate_request(purpose, Mode::Deidentified), Ok(()));
        assert_eq!(
            validate_request(purpose, Mode::Identified),
            Err(PolicyError::ConsentPurposeMismatch {
                purpose,
                mode: Mode::Identified
            })
        );
    }
}

#[test]
fn wire_names_are_snake_case() {
    let names: Vec<String> = [
        Purpose::ClinicalCare,
        Purpose::Research,
        Purpose::QualityImprovement,
        Purpose::PatientRequest,
    ]
    .iter()
    .map(|p| serde_json::to_string(p).unwrap())
    .collect();
    assert_eq!(
        names,
        [
            "\"clinical_care\"",
            "\"research\"",
            "\"quality_improvement\"",
            "\"patient_request\""
        ]
    );
    assert_eq!(
        serde_json::to_string(&Mode::Deidentified).unwrap(),
        "\"deidentified\""
    );
    assert_eq!(
        serde_json::to_string(&ExclusionReason::NoResearchConsent).unwrap(),
        "\"no_research_consent\""
    );
    assert_eq!(ExclusionReason::Withdrawn.as_str(), "withdrawn");
}

mod export_support;

#[test]
fn select_keeps_input_order_and_counts_exclusions_by_reason() {
    use asd_export::policy::select;
    let consented = export_support::golden_record();
    let mut withdrawn = export_support::golden_record();
    withdrawn.consent.withdrawn = true;
    let mut not_consented = export_support::golden_record();
    not_consented.consent.research_consented = Some(false);
    let mut unknown = export_support::golden_record();
    unknown.consent.research_consented = None;
    let records = [
        not_consented,
        consented.clone(),
        withdrawn,
        unknown,
        consented,
    ];

    let s = select(&records, Purpose::Research, Mode::Deidentified).unwrap();
    assert_eq!(s.included, [1, 4]);
    assert_eq!(s.excluded.get(&Withdrawn), Some(&1));
    assert_eq!(s.excluded.get(&NoResearchConsent), Some(&2));
    assert_eq!(s.excluded.get(&ModeNotAllowed), None);
    assert_eq!(
        serde_json::to_value(&s.excluded).unwrap(),
        serde_json::json!({"withdrawn": 1, "no_research_consent": 2})
    );

    let s = select(&records, Purpose::ClinicalCare, Mode::Identified).unwrap();
    assert_eq!(s.included, [0, 1, 3, 4]);
    assert_eq!(s.excluded.len(), 1);

    assert_eq!(
        select(&records, Purpose::Research, Mode::Identified).unwrap_err(),
        PolicyError::ConsentPurposeMismatch {
            purpose: Purpose::Research,
            mode: Mode::Identified
        }
    );
}

//! owner: WP-I.1 (table and gate); WP-I.3 / WP-I.4 add and verify the codes they use
//!
//! External code table (XC-7, ADR-0008 D8). Every external code (LOINC, SNOMED CT, HL7
//! terminology, IHE format codes, UCUM) an encoder emits lives here with a `verified` flag
//! and a source note. New encoders take codes through [`Code::require`], which refuses an
//! unverified code unless the org's integration profile opts in ([`CodePolicy`]).
//!
//! `verified = true` means someone checked code and display against the publisher's
//! release and wrote where in `source`. Everything else starts `false`, including codes the
//! iOS app has used for a long time.
//!
//! Exception, on purpose: the WP-B.8 session bundle (`fhir::session`) is a byte-exact port
//! of the iOS `FHIRBundleBuilder` (parity golden, ADR-0003) and keeps emitting the codes iOS
//! emits, verified or not (LOINC `93044-6`, the XFDF format code of G7). Its codes are
//! listed here (a test walks its golden) so the inventory is complete. WP-I.3 left it
//! unchanged: what it emits changes together with iOS (ADR-0008 Q6). The clinical bundle
//! (`fhir::clinical`) reuses its Observation and drops the unverified codings from it.
//!
//! Local codes under `http://curv.org/fhir/CodeSystem/...` (ADR-0003 D7) are ours and are
//! not listed.
//!
//! WP-I.3 verification (2026-10-31). Sources: the HL7 FHIR terminology server
//! `https://tx.fhir.org/r4/CodeSystem/$lookup` (LOINC 2.83, SNOMED CT International Edition
//! 2025-02-01, UCUM 2.2, THO `provenance-participant-type` 2.0.0, THO `v2-0203` 5.0.0) and
//! the FHIR R4 pages named per code. loinc.org refused automated access (HTTP 403), so LOINC
//! codes were checked against the LOINC release the HL7 terminology server publishes.
//! The lookups also refuted several iOS codes: LOINC `93044-6` is "Level of evidence", the
//! M-CHAT-R/F codes `80628-1`..`80650-5` are toxicology and drug-level tests, SNOMED CT
//! `713444003` does not exist, and `device` is not a `provenance-participant-type` code.
//! They stay `verified = false` with the finding in `source`; the iOS app and the session
//! bundle still emit them (ADR-0008 Q6, follow-up for iOS).

use std::fmt;

/// Code system URIs.
pub mod system {
    /// Base of every local URI (ADR-0003 D7). Codes under it are not in the table.
    pub const LOCAL: &str = "http://curv.org/fhir";
    pub const LOINC: &str = "http://loinc.org";
    pub const SNOMED: &str = "http://snomed.info/sct";
    pub const UCUM: &str = "http://unitsofmeasure.org";
    pub const OBSERVATION_CATEGORY: &str =
        "http://terminology.hl7.org/CodeSystem/observation-category";
    pub const DATA_OPERATION: &str = "http://terminology.hl7.org/CodeSystem/v3-DataOperation";
    pub const PROVENANCE_PARTICIPANT_TYPE: &str =
        "http://terminology.hl7.org/CodeSystem/provenance-participant-type";
    pub const ROLE_CODE: &str = "http://terminology.hl7.org/CodeSystem/v3-RoleCode";
    pub const V2_IDENTIFIER_TYPE: &str = "http://terminology.hl7.org/CodeSystem/v2-0203";
    pub const US_CORE_DOC_CATEGORY: &str =
        "http://hl7.org/fhir/us/core/CodeSystem/us-core-documentreference-category";
    pub const IHE_FORMAT: &str = "http://ihe.net/fhir/ValueSet/IHE.FormatCode.codesystem";
}

/// One external code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code {
    pub system: &'static str,
    pub code: &'static str,
    pub display: &'static str,
    /// Checked against the publisher's release (see `source`).
    pub verified: bool,
    /// Where the code comes from and, when verified, where it was checked.
    pub source: &'static str,
}

/// Whether unverified codes may be emitted: the integration profile's opt-in (XC-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CodePolicy {
    pub allow_unverified: bool,
}

/// An encoder asked for an unverified code without the opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("code {system}|{code} is not verified (XC-7) and the profile does not opt in")]
pub struct UnverifiedCode {
    pub system: &'static str,
    pub code: &'static str,
}

impl Code {
    /// The code, if `policy` allows emitting it.
    pub fn require(&self, policy: CodePolicy) -> Result<&Self, UnverifiedCode> {
        if self.verified || policy.allow_unverified {
            Ok(self)
        } else {
            Err(UnverifiedCode {
                system: self.system,
                code: self.code,
            })
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}|{} ({})", self.system, self.code, self.display)
    }
}

const IOS_PARITY: &str = "Emitted by the iOS FHIRBundleBuilder and the WP-B.8 parity bundle \
                          (ADR-0003); not checked against the publisher's release";

/// Source note prefix for codes checked on the HL7 FHIR terminology server (module docs).
macro_rules! tx_lookup {
    ($text:literal) => {
        concat!(
            "Checked 2026-10-31 (WP-I.3) with https://tx.fhir.org/r4/CodeSystem/$lookup: ",
            $text
        )
    };
}

// ---- LOINC ---------------------------------------------------------------------------------

/// G8 / ADR-0008 Q6: never sent by new encoders until verified.
pub const LOINC_GAZE_PANEL: Code = Code {
    system: system::LOINC,
    code: "93044-6",
    display: "Gaze tracking assessment panel",
    verified: false,
    source: concat!(
        "iOS ClinicalInteroperabilityStandards.swift:67, kept by ADR-0003 (G8, ADR-0008 Q6). ",
        tx_lookup!(
            "LOINC 2.83 names 93044-6 \"Level of evidence\", not a gaze panel: the iOS code \
             is wrong; do not verify it"
        )
    ),
};
pub const LOINC_MCHAT_TOTAL: Code = Code {
    system: system::LOINC,
    code: "80649-7",
    display: "M-CHAT-R total score [Score]",
    verified: false,
    source: concat!(
        "iOS parity bundle. ",
        tx_lookup!(
            "LOINC 2.83 names 80649-7 \"Tetramethylenedisulfotetramine [Mass/volume] in \
             Urine\": wrong code for M-CHAT"
        )
    ),
};
pub const LOINC_MCHAT_RISK: Code = Code {
    system: system::LOINC,
    code: "80650-5",
    display: "M-CHAT-R risk level [Interpretation]",
    verified: false,
    source: concat!(
        "iOS parity bundle. ",
        tx_lookup!(
            "LOINC 2.83 names 80650-5 \"Tin [Moles/volume] in Blood\": wrong code for M-CHAT"
        )
    ),
};
/// The M-CHAT-R/F questionnaire panel (`Questionnaire` canonical `http://loinc.org/q/80628-1`).
pub const LOINC_MCHAT_PANEL: Code = Code {
    system: system::LOINC,
    code: "80628-1",
    display: "M-CHAT-R/F panel",
    verified: false,
    source: concat!(
        "iOS StandardCodingSystems.LOINC (questionnaire canonical). ",
        tx_lookup!(
            "LOINC 2.83 names 80628-1 \"1,1'-Sulfonylbis-2-Methylthioethane [Mass/volume] in \
             Urine\": wrong code for M-CHAT"
        )
    ),
};
pub const LOINC_OUTPATIENT_NOTE: Code = Code {
    system: system::LOINC,
    code: "34108-1",
    display: "Outpatient Note",
    verified: true,
    source: concat!(
        "iOS DoctorReportGenerator DocumentReference.type (G7), parity bundle only. ",
        tx_lookup!("LOINC 2.83, active, display \"Outpatient Note\"")
    ),
};
/// `DocumentReference.type` of the PDF/A clinical summary (FHX-5).
pub const LOINC_SUMMARY_OF_EPISODE_NOTE: Code = Code {
    system: system::LOINC,
    code: "34133-9",
    display: "Summary of Episode Note",
    verified: true,
    source: tx_lookup!("LOINC 2.83, active, display \"Summary of Episode Note\""),
};
/// `DocumentReference.type` of a referral letter (FHX-5).
pub const LOINC_REFERRAL_NOTE: Code = Code {
    system: system::LOINC,
    code: "57133-1",
    display: "Referral Note",
    verified: true,
    source: tx_lookup!("LOINC 2.83, active, display \"Referral Note\""),
};

const MCHAT_ITEM_SOURCE: &str = concat!(
    "iOS StandardCodingSystems.LOINC M-CHAT-R/F item codes 1..20. ",
    tx_lookup!(
        "in LOINC 2.83 all twenty (80629-9..80648-9) are toxicology or drug-level tests \
         (e.g. 80629-9 \"1,2,2-Trimethylpropyl methylphosphonate [Mass/volume] in Blood\"): \
         wrong codes for M-CHAT items"
    )
);

macro_rules! mchat_items {
    ($($code:literal => $display:literal),* $(,)?) => {
        /// LOINC item codes of M-CHAT-R/F questions 1..=20, in order.
        pub const LOINC_MCHAT_ITEMS: [Code; 20] = [$(Code {
            system: system::LOINC,
            code: $code,
            display: $display,
            verified: false,
            source: MCHAT_ITEM_SOURCE,
        }),*];
    };
}

mchat_items! {
    "80629-9" => "M-CHAT-R/F question 1", "80630-7" => "M-CHAT-R/F question 2",
    "80631-5" => "M-CHAT-R/F question 3", "80632-3" => "M-CHAT-R/F question 4",
    "80633-1" => "M-CHAT-R/F question 5", "80634-9" => "M-CHAT-R/F question 6",
    "80635-6" => "M-CHAT-R/F question 7", "80636-4" => "M-CHAT-R/F question 8",
    "80637-2" => "M-CHAT-R/F question 9", "80638-0" => "M-CHAT-R/F question 10",
    "80639-8" => "M-CHAT-R/F question 11", "80640-6" => "M-CHAT-R/F question 12",
    "80641-4" => "M-CHAT-R/F question 13", "80642-2" => "M-CHAT-R/F question 14",
    "80643-0" => "M-CHAT-R/F question 15", "80644-8" => "M-CHAT-R/F question 16",
    "80645-5" => "M-CHAT-R/F question 17", "80646-3" => "M-CHAT-R/F question 18",
    "80647-1" => "M-CHAT-R/F question 19", "80648-9" => "M-CHAT-R/F question 20",
}

// ---- SNOMED CT -----------------------------------------------------------------------------

pub const SNOMED_EYE_TRACKING_EXAM: Code = Code {
    system: system::SNOMED,
    code: "713444003",
    display: "Eye tracking examination (procedure)",
    verified: false,
    source: concat!(
        "iOS parity bundle. ",
        tx_lookup!(
            "unknown code in SNOMED CT International Edition 2025-02-01: the iOS code does \
             not exist"
        )
    ),
};

// ---- HL7 terminology -----------------------------------------------------------------------

const OBSERVATION_CATEGORY_SOURCE: &str = "Verified 2026-10-31 (WP-I.1) against \
     https://hl7.org/fhir/R4/codesystem-observation-category.html";

pub const OBSERVATION_CATEGORY_EXAM: Code = Code {
    system: system::OBSERVATION_CATEGORY,
    code: "exam",
    display: "Exam",
    verified: true,
    source: OBSERVATION_CATEGORY_SOURCE,
};
pub const OBSERVATION_CATEGORY_SURVEY: Code = Code {
    system: system::OBSERVATION_CATEGORY,
    code: "survey",
    display: "Survey",
    verified: true,
    source: OBSERVATION_CATEGORY_SOURCE,
};
pub const PROVENANCE_PARTICIPANT_DEVICE: Code = Code {
    system: system::PROVENANCE_PARTICIPANT_TYPE,
    code: "device",
    display: "Device",
    verified: false,
    source: concat!(
        "iOS parity bundle. ",
        tx_lookup!(
            "unknown code in provenance-participant-type 2.0.0, and not in the R4 value set \
             https://hl7.org/fhir/R4/valueset-provenance-agent-type.html either"
        )
    ),
};
const PROVENANCE_PARTICIPANT_SOURCE: &str = "Verified 2026-10-31 (WP-I.3) against \
     https://hl7.org/fhir/R4/valueset-provenance-agent-type.html and tx.fhir.org $lookup \
     (provenance-participant-type 2.0.0)";
/// Provenance agent type of the first signature (FHX-2).
pub const PROVENANCE_PARTICIPANT_AUTHOR: Code = Code {
    system: system::PROVENANCE_PARTICIPANT_TYPE,
    code: "author",
    display: "Author",
    verified: true,
    source: PROVENANCE_PARTICIPANT_SOURCE,
};
/// Provenance agent type of the second signature (cosign, FHX-2).
pub const PROVENANCE_PARTICIPANT_VERIFIER: Code = Code {
    system: system::PROVENANCE_PARTICIPANT_TYPE,
    code: "verifier",
    display: "Verifier",
    verified: true,
    source: PROVENANCE_PARTICIPANT_SOURCE,
};
pub const DATA_OPERATION_CREATE: Code = Code {
    system: system::DATA_OPERATION,
    code: "CREATE",
    display: "create",
    verified: false,
    source: IOS_PARITY,
};
pub const ROLE_CODE_PARENT: Code = Code {
    system: system::ROLE_CODE,
    code: "PRN",
    display: "parent",
    verified: false,
    source: IOS_PARITY,
};
/// `Patient.identifier.type` for the MRN (FHX-3) and HL7 `PID-3` type `MR` (HL7-3).
pub const V2_IDENTIFIER_TYPE_MR: Code = Code {
    system: system::V2_IDENTIFIER_TYPE,
    code: "MR",
    display: "Medical record number",
    verified: true,
    source: "HL7 v2 table 0203 (Identifier Type), FHX-3 and HL7-3. Verified 2026-10-31 \
             (WP-I.3) against https://hl7.org/fhir/R4/v2/0203/index.html (system \
             http://terminology.hl7.org/CodeSystem/v2-0203, MR \"Medical record number\") and \
             tx.fhir.org $lookup (v2-0203 5.0.0)",
};
pub const US_CORE_CLINICAL_NOTE: Code = Code {
    system: system::US_CORE_DOC_CATEGORY,
    code: "clinical-note",
    display: "Clinical Note",
    verified: false,
    source: "iOS DoctorReportGenerator DocumentReference.category (US Core, G7); parity \
             bundle only",
};

// ---- IHE -----------------------------------------------------------------------------------

/// G7: this is the XFDF format code, not PDF. Parity bundle only; new encoders use
/// [`IHE_FORMAT_MIME_TYPE_SUFFICIENT`].
pub const IHE_FORMAT_XFDF: Code = Code {
    system: system::IHE_FORMAT,
    code: "urn:ihe:iti:xfdf:2008",
    display: "PDF Document",
    verified: false,
    source: "iOS DoctorReportGenerator.swift:490-568; wrong for a PDF attachment (G7) and \
             not in the R4 format-code value set \
             (https://hl7.org/fhir/R4/valueset-formatcodes.html, checked 2026-10-31, WP-I.3)",
};
/// `DocumentReference.content.format` of a generated PDF whose MIME type says all a reader
/// needs (FHX-5, fixes G7).
pub const IHE_FORMAT_MIME_TYPE_SUFFICIENT: Code = Code {
    system: system::IHE_FORMAT,
    code: "urn:ihe:iti:xds:2017:mimeTypeSufficient",
    display: "mimeType Sufficient",
    verified: true,
    source: "Verified 2026-10-31 (WP-I.3) against \
             https://hl7.org/fhir/R4/valueset-formatcodes.html (system \
             http://ihe.net/fhir/ValueSet/IHE.FormatCode.codesystem, display \"mimeType \
             Sufficient\"). urn:ihe:iti:xds-sd:pdf:2008 is for scanned documents (XDS-SD) and \
             does not fit a generated PDF",
};

// ---- UCUM ----------------------------------------------------------------------------------

/// Dimensionless quantities (the gaze Observation components).
pub const UCUM_UNITY: Code = Code {
    system: system::UCUM,
    code: "1",
    display: "unity",
    verified: true,
    source: tx_lookup!(
        "UCUM 2.2, code `1` is valid (UCUM defines no display names; its print symbol is \
         `1` and `unity` is a descriptive label)"
    ),
};
pub const UCUM_MONTH: Code = Code {
    system: system::UCUM,
    code: "mo",
    display: "month",
    verified: false,
    source: IOS_PARITY,
};

/// Every code in the table.
pub static ALL: &[&Code] = &[
    &LOINC_GAZE_PANEL,
    &LOINC_MCHAT_TOTAL,
    &LOINC_MCHAT_RISK,
    &LOINC_MCHAT_PANEL,
    &LOINC_OUTPATIENT_NOTE,
    &LOINC_SUMMARY_OF_EPISODE_NOTE,
    &LOINC_REFERRAL_NOTE,
    &LOINC_MCHAT_ITEMS[0],
    &LOINC_MCHAT_ITEMS[1],
    &LOINC_MCHAT_ITEMS[2],
    &LOINC_MCHAT_ITEMS[3],
    &LOINC_MCHAT_ITEMS[4],
    &LOINC_MCHAT_ITEMS[5],
    &LOINC_MCHAT_ITEMS[6],
    &LOINC_MCHAT_ITEMS[7],
    &LOINC_MCHAT_ITEMS[8],
    &LOINC_MCHAT_ITEMS[9],
    &LOINC_MCHAT_ITEMS[10],
    &LOINC_MCHAT_ITEMS[11],
    &LOINC_MCHAT_ITEMS[12],
    &LOINC_MCHAT_ITEMS[13],
    &LOINC_MCHAT_ITEMS[14],
    &LOINC_MCHAT_ITEMS[15],
    &LOINC_MCHAT_ITEMS[16],
    &LOINC_MCHAT_ITEMS[17],
    &LOINC_MCHAT_ITEMS[18],
    &LOINC_MCHAT_ITEMS[19],
    &SNOMED_EYE_TRACKING_EXAM,
    &OBSERVATION_CATEGORY_EXAM,
    &OBSERVATION_CATEGORY_SURVEY,
    &PROVENANCE_PARTICIPANT_DEVICE,
    &PROVENANCE_PARTICIPANT_AUTHOR,
    &PROVENANCE_PARTICIPANT_VERIFIER,
    &DATA_OPERATION_CREATE,
    &ROLE_CODE_PARENT,
    &V2_IDENTIFIER_TYPE_MR,
    &US_CORE_CLINICAL_NOTE,
    &IHE_FORMAT_XFDF,
    &IHE_FORMAT_MIME_TYPE_SUFFICIENT,
    &UCUM_UNITY,
    &UCUM_MONTH,
];

/// The table entry for `system|code`.
pub fn lookup(system: &str, code: &str) -> Option<&'static Code> {
    ALL.iter()
        .copied()
        .find(|c| c.system == system && c.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mchat_item_codes_match_the_bundle_builder() {
        let table: Vec<&str> = LOINC_MCHAT_ITEMS.iter().map(|c| c.code).collect();
        assert_eq!(table, crate::mchat::QUESTION_CODES);
    }
}

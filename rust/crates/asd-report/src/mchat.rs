//! M-CHAT-R/F result as the report builders need it (`MCHATScoreResult` in Swift).
//!
//! The backend has no M-CHAT source yet; the type is here so the FHIR
//! `QuestionnaireResponse` + score `Observation` (FHIR-4) and the PDF section are one
//! implementation for every caller that does.

use serde::{Deserialize, Serialize};

/// Official M-CHAT-R/F scoring category. Serialised with the iOS persisted raw values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MchatRiskLevel {
    #[serde(rename = "Low Risk")]
    Low,
    #[serde(rename = "Medium Risk")]
    Medium,
    #[serde(rename = "High Risk")]
    High,
}

impl MchatRiskLevel {
    /// Category text for LOINC `80650-5` (official scoring category, FHIR-4).
    pub fn category_text(self) -> &'static str {
        match self {
            Self::Low => "Low-Risk (total score 0-2)",
            Self::Medium => "Medium-Risk (total score 3-7)",
            Self::High => "High-Risk (total score 8-20)",
        }
    }
}

/// A completed questionnaire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MchatResult {
    /// Items failed (LOINC `80649-7`).
    pub total_failed: i64,
    pub risk_level: MchatRiskLevel,
    /// 1-based ids of the failed items, in order.
    pub failed_question_ids: Vec<u8>,
    /// RFC 3339.
    pub completion_date: String,
}

/// LOINC item codes for M-CHAT-R/F questions 1..=20 (`StandardCodingSystems.LOINC`).
pub(crate) const QUESTION_CODES: [&str; 20] = [
    "80629-9", "80630-7", "80631-5", "80632-3", "80633-1", "80634-9", "80635-6", "80636-4",
    "80637-2", "80638-0", "80639-8", "80640-6", "80641-4", "80642-2", "80643-0", "80644-8",
    "80645-5", "80646-3", "80647-1", "80648-9",
];

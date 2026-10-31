//! Mandatory report copy (CLM-4, CLM-5), verbatim from the iOS `ClaimsCopy` and
//! `MCHATRiskLevel.displayName`. Reports state measurements only: no diagnosis, no risk
//! score, no "screening" wording (claims denylist, ADR-0004).

use crate::mchat::MchatRiskLevel;

/// "This is not a diagnostic tool." (CLM-5)
pub const NOT_DIAGNOSTIC: &str = "This is not a diagnostic tool.";
/// Mission line (CLM-5).
pub const MISSION: &str = "Part of Open Spectrum Tracking";
/// Under the per-clip table.
pub const AGE_REFERENCE_NOTE: &str =
    "Age references are provisional and drawn from published literature. They are not a diagnosis.";

/// "M-CHAT-R/F: <level> (<official next step>)" (CLM-4): always names the instrument and
/// never shows the persisted raw value ("High Risk").
pub fn mchat_line(level: MchatRiskLevel) -> &'static str {
    match level {
        MchatRiskLevel::Low => {
            "M-CHAT-R/F: low (no follow-up needed now; discuss any concerns with your pediatrician)"
        }
        MchatRiskLevel::Medium => {
            "M-CHAT-R/F: medium (follow-up interview needed; discuss with your pediatrician)"
        }
        MchatRiskLevel::High => {
            "M-CHAT-R/F: high (follow-up needed; discuss with your pediatrician soon)"
        }
    }
}

import SwiftUI
import CurvAsdGazeKit

// MARK: - Mandatory claims copy (ADR-0004 decision 5, CLM-4, CLM-5)

/// Single source for the sentences that must appear on every results surface
/// (session result, dashboard card, PDF, store description).
public enum ClaimsCopy {
    /// "This is not a diagnostic tool." (CLM-5)
    public static let notDiagnostic = "This is not a diagnostic tool."
    /// Mission line (CLM-5)
    public static let mission = "Part of Open Spectrum Tracking"

    /// "M-CHAT-R/F: <level> (<official next step>)" for a questionnaire result (CLM-4).
    /// Always names the instrument and never shows the bare persisted raw value.
    public static func mchatLine(for level: MCHATRiskLevel) -> String {
        let name = level.displayName
        if name.hasPrefix("M-CHAT-R/F") { return name }
        if name.contains("(") { return "M-CHAT-R/F: \(name)" }
        return "M-CHAT-R/F: \(name) (\(mchatNextStep(for: level)))"
    }

    /// Official next step wording for each M-CHAT-R/F category.
    public static func mchatNextStep(for level: MCHATRiskLevel) -> String {
        switch level {
        case .low: return "no follow-up needed unless you have concerns; discuss with your pediatrician"
        case .medium: return "follow-up interview needed; discuss with your pediatrician"
        case .high: return "follow-up needed; discuss with your pediatrician soon"
        }
    }
}

// MARK: - Footer

/// Persistent footer for every results surface: the non-diagnostic sentence and the mission line.
public struct NotDiagnosticFooter: View {
    public init() {}

    public var body: some View {
        VStack(spacing: 2) {
            Text(ClaimsCopy.notDiagnostic)
                .font(.system(size: 11, weight: .semibold, design: .rounded))
                .foregroundColor(LiquidGlass.textSecondary)
            Text(ClaimsCopy.mission)
                .font(.system(size: 10, weight: .medium))
                .foregroundColor(LiquidGlass.textTertiary)
        }
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .accessibilityElement(children: .combine)
    }
}

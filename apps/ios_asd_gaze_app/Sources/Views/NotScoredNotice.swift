import CurvAsdGazeKit
import SwiftUI

/// Shown where score values would be for an `unscorable` session (SCH-5): no number is
/// invented for a score that was not computed (CON-14).
struct NotScoredNotice: View {
    let reason: UnscorableReason?

    static func text(for reason: UnscorableReason?) -> String {
        switch reason {
        case .insufficientValidGaze?: return "Not scored: too few usable gaze samples."
        case .aggregationFailed?: return "Not scored: the recording could not be processed."
        case .trackingUnusable?: return "Not scored: eye tracking was not reliable enough."
        case .calibrationFailed?: return "Not scored: calibration did not succeed."
        case .noScorableClips?: return "Not scored: no clip could be scored."
        case .other?, nil: return "Not scored."
        }
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.circle")
                .foregroundColor(LiquidGlass.warmAmber)
            Text(Self.text(for: reason) + " Run the session again.")
                .font(.system(size: 13, weight: .medium))
                .foregroundColor(.primary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(cornerRadius: 16, padding: 14)
        .accessibilityElement(children: .combine)
    }
}

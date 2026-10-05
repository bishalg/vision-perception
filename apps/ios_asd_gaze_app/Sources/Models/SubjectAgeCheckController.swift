import Foundation
import SwiftUI
import CurvAsdGazeKit

// MARK: - Cross-Check Verdict

/// Outcome of comparing the on-camera age-band estimate against the caregiver-entered
/// `PediatricAgeBracket`. Assistive only: a mismatch produces a soft confirm prompt,
/// never a block.
enum SubjectAgeCheckVerdict: Equatable {
    /// Not enough stable evidence yet (no signal, or the window hasn't stabilized).
    case insufficientData
    /// Estimate is within one band of the picked bracket — no action needed.
    case consistent
    /// Estimate is ≥ 2 bands from the picked bracket (e.g. toddler pick, adult face).
    case softConfirm(bandsApart: Int)
}

// MARK: - Subject Age Check Controller

/// Pure decision logic + presentation state for the assistive subject-age feature.
/// The heavy lifting (ARKit frames → crop → CoreML → windowed signal) lives in
/// `CurvAsdGazeKit.SubjectAgeEstimator`; this controller turns a signal into UX
/// verdicts and keeps the soft-prompt dismissal state.
@MainActor
final class SubjectAgeCheckController: ObservableObject {
    @Published private(set) var lastSignal: SubjectAgeSignal?
    @Published private(set) var dismissedSoftConfirm: Bool = false

    /// Compares an estimated band against the picked age bracket in months.
    /// A stable signal ≥ 2 bands away triggers a soft confirm; one band of
    /// tolerance absorbs the known overestimation bias on young children.
    static func crossCheckVerdict(
        estimated: FacialAgeBand?,
        isStable: Bool,
        pickedMonths: Int
    ) -> SubjectAgeCheckVerdict {
        guard let estimated, isStable else { return .insufficientData }
        let pickedBand = FacialAgeBand.approximateBand(months: pickedMonths)
        let bandsApart = estimated.ordinalDistance(to: pickedBand)
        return bandsApart >= 2 ? .softConfirm(bandsApart: bandsApart) : .consistent
    }

    /// True when a stable teen/adult face dominates the window while the app
    /// expects the child in front of the camera (positioning hint, session guard).
    static func shouldShowAdultHoldingHint(signal: SubjectAgeSignal?) -> Bool {
        guard let signal, signal.isStable else { return false }
        return signal.band == .teenAdult
    }

    /// Human-readable summary line for the current signal (nil when insufficient).
    static func statusLine(for signal: SubjectAgeSignal?) -> String? {
        guard let signal, signal.isStable else { return nil }
        let percent = Int(round(signal.confidence * 100))
        return "\(signal.band.label) • \(percent)% model confidence"
    }

    var currentVerdict: SubjectAgeCheckVerdict {
        Self.crossCheckVerdict(
            estimated: lastSignal?.band,
            isStable: lastSignal?.isStable ?? false,
            pickedMonths: ChildProfileStore.shared.profile.ageBracket.midpointMonths
        )
    }

    func observe(_ signal: SubjectAgeSignal?) {
        lastSignal = signal
    }

    func dismissSoftConfirm() {
        dismissedSoftConfirm = true
    }

    func reset() {
        lastSignal = nil
        dismissedSoftConfirm = false
    }
}

import CoreGraphics

/// Bookkeeping for the bubble calibration (CAL-3): which targets produced real samples.
///
/// No arithmetic lives here. The caller passes the per-target robust raw point, or
/// `nil` when the target collected no samples. A target with no samples is shown once
/// more; if it is starved again the calibration fails visibly. A target's own position
/// is never substituted for a missing measurement.
struct CalibrationAccumulator {
    enum Outcome: Equatable {
        /// The target has a real measurement. `isComplete` once every target has one.
        case accepted(isComplete: Bool)
        /// No samples: show the same target again.
        case retryTarget
        /// No samples after the retry: stop and offer "try again".
        case failed
    }

    static let maxRetriesPerTarget = 1

    let targetCount: Int
    private(set) var rawPoints: [CGPoint] = []
    private var retriesForCurrentTarget = 0
    private(set) var hasFailed = false

    init(targetCount: Int) {
        self.targetCount = targetCount
    }

    /// True only when every target contributed a measured point and nothing failed.
    var canFit: Bool { !hasFailed && rawPoints.count == targetCount }

    mutating func submit(medianRaw: CGPoint?) -> Outcome {
        guard !hasFailed else { return .failed }
        guard let medianRaw else {
            if retriesForCurrentTarget < Self.maxRetriesPerTarget {
                retriesForCurrentTarget += 1
                return .retryTarget
            }
            hasFailed = true
            return .failed
        }
        rawPoints.append(medianRaw)
        retriesForCurrentTarget = 0
        return .accepted(isComplete: rawPoints.count == targetCount)
    }

    /// The fit could not be computed from complete data: fail visibly, never skip silently.
    mutating func markFitFailed() {
        hasFailed = true
    }
}

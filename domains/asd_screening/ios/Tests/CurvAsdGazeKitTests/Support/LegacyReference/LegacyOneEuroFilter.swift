// Former Swift body of `OneEuroFilter` / `OneEuroFilter2D`, moved here by WP-G.9 (PIPE-3): the
// 1-Euro filter now runs in the Rust pipeline (`asd-core::gaze::filter`, matched to this code
// to 1e-12 by the `one_euro_swift_v1` golden). This copy is the reference for one release
// (OneEuroFilterTests). Do not change its behaviour.
import Foundation
import CoreGraphics

// MARK: - 1 Euro Filter (Casiez et al., CHI 2012)
//
// Adaptive low-pass filter designed specifically for noisy 60 Hz positional signals
// like eye gaze. It solves the fundamental trade-off between jitter reduction and
// motion lag by making the filter cutoff frequency a function of signal velocity:
//
//   • At rest (fixation): very low cutoff (~1 Hz) — eliminates micro-tremors completely.
//   • On fast movement (saccade): high cutoff — eliminates lag so the cursor
//     reaches its target instantly without "smearing" or trailing.
//
// Parameters (tunable):
//   minCutoff (Hz):  Cutoff at zero velocity. Lower = smoother fixation, higher = more jitter.
//   beta:            Speed coefficient. Higher = faster saccade tracking, lower = more stable.
//   dCutoff (Hz):    Cutoff for the velocity estimate itself (avoids derivative noise amplification).

/// One-dimensional 1 Euro Filter for a single scalar signal stream.
final class LegacyOneEuroFilter: @unchecked Sendable {
    var minCutoff: Double
    var beta: Double
    var dCutoff: Double

    private var xFilt: LegacyLowPassFilter
    private var dxFilt: LegacyLowPassFilter
    private var lastTimestamp: Double?
    private let lock = NSLock()

    /// - Parameters:
    ///   - minCutoff: Minimum cutoff frequency in Hz. Default 1.2 Hz balances jitter vs responsiveness.
    ///   - beta: Speed coefficient — controls how fast the cutoff rises with velocity. Default 0.015.
    ///   - dCutoff: Cutoff for the derivative (velocity) estimate. Default 1.0 Hz.
    init(minCutoff: Double = 1.2, beta: Double = 0.015, dCutoff: Double = 1.0) {
        self.minCutoff = minCutoff
        self.beta = beta
        self.dCutoff = dCutoff
        self.xFilt = LegacyLowPassFilter(alpha: 1.0)
        self.dxFilt = LegacyLowPassFilter(alpha: 1.0)
    }

    /// Processes the next raw sample and returns the filtered value.
    /// - Parameters:
    ///   - value: The raw noisy signal value.
    ///   - timestamp: Wall-clock timestamp in seconds (e.g. from `CACurrentMediaTime()`).
    /// - Returns: The filtered value.
    func filter(value: Double, timestamp: Double) -> Double {
        lock.lock()
        defer { lock.unlock() }

        let dt: Double
        if let last = lastTimestamp {
            dt = max(timestamp - last, 1.0 / 240.0) // clamp to prevent dt=0 spikes
        } else {
            dt = 1.0 / 60.0
        }
        lastTimestamp = timestamp

        // Derivative estimate
        let dx = xFilt.lastRaw.map { (value - $0) / dt } ?? 0.0
        let edx = dxFilt.filter(value: dx, alpha: alpha(cutoff: dCutoff, dt: dt))

        // Adaptive cutoff: rises with velocity so saccades are tracked instantly
        let cutoff = minCutoff + beta * abs(edx)
        return xFilt.filter(value: value, alpha: alpha(cutoff: cutoff, dt: dt))
    }

    /// Resets the filter state (call when switching to a new tracking session).
    func reset() {
        lock.lock()
        defer { lock.unlock() }

        xFilt = LegacyLowPassFilter(alpha: 1.0)
        dxFilt = LegacyLowPassFilter(alpha: 1.0)
        lastTimestamp = nil
    }

    // Compute the low-pass alpha from Hz cutoff and frame dt.
    private func alpha(cutoff: Double, dt: Double) -> Double {
        let te = 1.0 / max(cutoff, 0.001)
        let tau = te / (2.0 * .pi)
        return 1.0 / (1.0 + tau / dt)
    }
}

// MARK: - 2D Wrapper

/// Two-dimensional 1 Euro Filter for screen-coordinate gaze positions.
/// Filters x and y axes independently with shared parameters.
final class LegacyOneEuroFilter2D: @unchecked Sendable {
    private let filterX: LegacyOneEuroFilter
    private let filterY: LegacyOneEuroFilter

    /// - Parameters:
    ///   - minCutoff: Minimum cutoff frequency in Hz. Default 1.2 Hz.
    ///   - beta: Speed coefficient. Default 0.015 — tuned for 60 Hz TrueDepth gaze at 40–50 cm.
    ///   - dCutoff: Derivative cutoff in Hz. Default 1.0 Hz.
    init(minCutoff: Double = 1.2, beta: Double = 0.015, dCutoff: Double = 1.0) {
        self.filterX = LegacyOneEuroFilter(minCutoff: minCutoff, beta: beta, dCutoff: dCutoff)
        self.filterY = LegacyOneEuroFilter(minCutoff: minCutoff, beta: beta, dCutoff: dCutoff)
    }

    /// Filters a raw gaze point and returns the smoothed result.
    /// - Parameters:
    ///   - point: Raw normalized gaze point (x, y in [0, 1]).
    ///   - timestamp: Wall-clock time in seconds (use `CACurrentMediaTime()`).
    func filter(point: CGPoint, timestamp: Double) -> CGPoint {
        CGPoint(
            x: filterX.filter(value: Double(point.x), timestamp: timestamp),
            y: filterY.filter(value: Double(point.y), timestamp: timestamp)
        )
    }

    /// Resets both axis filters — call at the start of a new ARKit session.
    func reset() {
        filterX.reset()
        filterY.reset()
    }
}

// MARK: - Internal Low-Pass Filter

/// Simple exponential moving average filter — used internally by `LegacyOneEuroFilter`.
private final class LegacyLowPassFilter {
    private(set) var lastRaw: Double?
    private var lastFiltered: Double?
    private var alpha: Double

    init(alpha: Double) {
        self.alpha = alpha
    }

    @discardableResult
    func filter(value: Double, alpha: Double) -> Double {
        self.alpha = alpha
        let result: Double
        if let prev = lastFiltered {
            result = alpha * value + (1.0 - alpha) * prev
        } else {
            result = value
        }
        lastRaw = value
        lastFiltered = result
        return result
    }
}

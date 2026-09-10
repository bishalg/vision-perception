import Foundation
import CoreMotion
import Combine

/**
 * Device roll vs gravity (camera axis tilt).
 * rollDeg = atan2(gravity.x, -gravity.y) in degrees, 0 = device vertical.
 * Mirrors Android DeviceTiltMonitor so both platforms report same sign.
 */
@MainActor
public final class DeviceTiltProvider: ObservableObject {
    @Published public var deviceRollDeg: Float = 0
    @Published public var isAvailable: Bool = false

    private let motion = CMMotionManager()
    private var timer: AnyCancellable?

    public init() {}

    public func start() {
        guard motion.isDeviceMotionAvailable else { return }
        motion.deviceMotionUpdateInterval = 1.0 / 15.0
        motion.startDeviceMotionUpdates(using: .xArbitraryZVertical)
        isAvailable = true
        // Poll gravity at display rate; CMMotion delivers on its own queue.
        timer = Timer.publish(every: 1.0/15.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                guard let self, let dm = self.motion.deviceMotion else { return }
                let g = dm.gravity
                // Same math as Android: roll from gravity vector
                let roll = atan2(g.x, -g.y) * 180.0 / Double.pi
                // Low-pass to avoid jitter
                let filtered = Float(roll) * 0.35 + self.deviceRollDeg * 0.65
                // Clamp to -45..45 for rail display, but keep true value
                self.deviceRollDeg = filtered
            }
    }

    public func stop() {
        timer?.cancel()
        timer = nil
        motion.stopDeviceMotionUpdates()
        isAvailable = false
    }
}

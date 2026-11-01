import Foundation

/// Identifies the one participant and session a calibration belongs to (CAL-1).
///
/// A fitted profile is only ever applied when the binding of the session that wants
/// it equals the binding it was saved under. A different child, or a later session,
/// gets the identity profile, never someone else's fit.
public struct CalibrationBinding: Codable, Sendable, Equatable, Hashable {
    /// Stable, pseudonymous key of the child being observed (never a name).
    public let participantKey: String
    /// Identifier of the screening session the calibration was made for.
    public let sessionId: String

    public init(participantKey: String, sessionId: String) {
        self.participantKey = participantKey
        self.sessionId = sessionId
    }
}

/// Persistence for the calibration of the current session and participant.
///
/// Replaces the global single-profile store as the controller's source: `load(for:)`
/// returns `.identity` unless the stored envelope carries exactly the requested
/// binding. Saving also purges the legacy device-wide profile (`GazeCalibrationStore`).
public enum BoundCalibrationStore {
    private static let key = "CurvAsdGazeKit.boundCalibration.v2"

    private struct Envelope: Codable {
        let binding: CalibrationBinding
        let profile: GazeCalibrationProfile
    }

    public static func save(
        _ profile: GazeCalibrationProfile,
        binding: CalibrationBinding,
        defaults: UserDefaults = .standard
    ) {
        GazeCalibrationStore.reset()
        guard let data = try? JSONEncoder().encode(Envelope(binding: binding, profile: profile)) else { return }
        defaults.set(data, forKey: key)
    }

    /// The profile saved for exactly `binding`, otherwise `.identity`.
    public static func load(
        for binding: CalibrationBinding?,
        defaults: UserDefaults = .standard
    ) -> GazeCalibrationProfile {
        guard let binding,
              let data = defaults.data(forKey: key),
              let envelope = try? JSONDecoder().decode(Envelope.self, from: data),
              envelope.binding == binding
        else { return .identity }
        return envelope.profile
    }

    public static var hasStoredProfile: Bool {
        UserDefaults.standard.data(forKey: key) != nil
    }

    public static func reset(defaults: UserDefaults = .standard) {
        defaults.removeObject(forKey: key)
        GazeCalibrationStore.reset()
    }
}

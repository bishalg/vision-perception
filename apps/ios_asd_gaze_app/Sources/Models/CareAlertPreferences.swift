import Foundation
import SwiftUI

/// Master caregiver toggles for medication and diary alerts (shared architecture).
@MainActor
public final class CareAlertPreferences: ObservableObject {
    public static let shared = CareAlertPreferences()

    private let masterKey = "org.curv.asd.care.alerts.master"
    private let alarmKitKey = "org.curv.asd.care.alerts.alarmkit"
    private let liveActivityKey = "org.curv.asd.care.alerts.liveactivity"

    @Published public var masterEnabled: Bool {
        didSet { UserDefaults.standard.set(masterEnabled, forKey: masterKey) }
    }

    @Published public var useAlarmKitWhenAvailable: Bool {
        didSet { UserDefaults.standard.set(useAlarmKitWhenAvailable, forKey: alarmKitKey) }
    }

    /// Opt-in Dynamic Island / Lock Screen countdown.
    @Published public var useLiveActivityCountdown: Bool {
        didSet { UserDefaults.standard.set(useLiveActivityCountdown, forKey: liveActivityKey) }
    }

    public init() {
        self.masterEnabled = UserDefaults.standard.object(forKey: masterKey) as? Bool ?? true
        self.useAlarmKitWhenAvailable = UserDefaults.standard.object(forKey: alarmKitKey) as? Bool ?? false
        self.useLiveActivityCountdown = UserDefaults.standard.object(forKey: liveActivityKey) as? Bool ?? false
    }

    public static var isAlarmKitAvailable: Bool {
        #if canImport(AlarmKit)
        if #available(iOS 26.0, *) { return true }
        #endif
        return false
    }
}

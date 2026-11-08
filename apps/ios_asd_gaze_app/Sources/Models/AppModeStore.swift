import Foundation
import SwiftUI

// MARK: - App Mode

/// The operational context chosen at first launch: personal home use by a parent/caregiver,
/// or shared-device professional use in a clinic / care institution.
///
/// The two modes share ~90% of the app; the mode today drives onboarding copy, the dashboard
/// presentation, and (as the profile/RBAC system ships) role gating. Persisted in UserDefaults;
/// switchable later from Settings.
public enum AppMode: String, Codable, CaseIterable, Sendable, Identifiable {
    /// Home use: parent or legal caregiver tracks their own child(ren).
    case parentHome = "parent_home"
    /// Institutional use: healthcare professionals screening children on a shared device.
    case clinic = "clinic"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .parentHome: return "Parent & Caregiver"
        case .clinic: return "Clinic / Professional"
        }
    }

    public var shortName: String {
        switch self {
        case .parentHome: return "Parent"
        case .clinic: return "Clinic"
        }
    }

    public var systemImage: String {
        switch self {
        case .parentHome: return "figure.2.and.child.holdinghands"
        case .clinic: return "cross.case.fill"
        }
    }

    public var tagline: String {
        switch self {
        case .parentHome:
            return "Record short gaze observation sessions at home and keep daily notes in one place."
        case .clinic:
            return "Record observation sessions for several children on a shared device and deliver them to your clinic's console."
        }
    }

    public var details: [String] {
        switch self {
        case .parentHome:
            return [
                "Guided 30–60 second gaze observations at home",
                "Daily care tasks, reminders, and mood diary",
                "Sessions delivered to your clinic after each recording",
                "Optional normative data contribution"
            ]
        case .clinic:
            return [
                "Multi-child observation on one shared device",
                "Session notes and delivery to your clinic's console",
                "De-identified participant codes by default",
                "Role-based access controls (rolling out)"
            ]
        }
    }
}

// MARK: - Store

/// Persists the selected `AppMode` and whether the first-launch choice has been made.
/// `@MainActor` ObservableObject so entry-level UI reacts to mode changes from Settings.
@MainActor
public final class AppModeStore: ObservableObject {
    public static let shared = AppModeStore()

    static let modeKey = "org.curv.asd.app_mode"
    static let selectionKey = "org.curv.asd.app_mode_selected"

    @Published public private(set) var mode: AppMode
    @Published public private(set) var hasSelectedMode: Bool

    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let raw = defaults.string(forKey: Self.modeKey),
           let selected = AppMode(rawValue: raw) {
            self.mode = selected
        } else {
            self.mode = .parentHome
        }
        // Defaults to false so existing installs see the mode choice exactly once after update.
        self.hasSelectedMode = defaults.bool(forKey: Self.selectionKey)
    }

    /// Selects a mode and marks the first-launch choice as made.
    public func select(_ newMode: AppMode) {
        mode = newMode
        hasSelectedMode = true
        defaults.set(newMode.rawValue, forKey: Self.modeKey)
        defaults.set(true, forKey: Self.selectionKey)
    }
}

import SwiftUI

public enum CameraDeniedAccessibility {
    public static let view = "camera-denied-view"
    public static let openSettings = "camera-denied-open-settings"
}

/// Plain-language copy for `CameraDeniedView` (CAM-1). No diagnostic wording.
public struct CameraDeniedContent: Equatable {
    public let title: String
    public let message: String
    public let offersOpenSettings: Bool

    public static func make(for state: CameraAccessState) -> CameraDeniedContent {
        if state == .restricted {
            return .init(
                title: "Camera is restricted on this device",
                message: "Camera use is limited by Screen Time or a device management profile, so gaze observations cannot run. You can still use the diary, reminders and notes.",
                offersOpenSettings: false)
        }
        return .init(
            title: "Camera access is turned off",
            message: "Gaze observations need the front camera. Turn on Camera for this app in Settings. Frames are analysed on the device and are not recorded as video.",
            offersOpenSettings: true)
    }
}

/// Shown when camera access is denied or restricted. Starts nothing.
public struct CameraDeniedView: View {
    public let state: CameraAccessState
    public var onClose: (() -> Void)?

    public init(state: CameraAccessState, onClose: (() -> Void)? = nil) {
        self.state = state
        self.onClose = onClose
    }

    public var body: some View {
        let content = CameraDeniedContent.make(for: state)
        ZStack {
            LiquidGlass.deepCanvas.ignoresSafeArea()
            VStack(spacing: 16) {
                Image(systemName: "camera.fill")
                    .font(.system(size: 44, weight: .semibold))
                    .foregroundColor(LiquidGlass.warmAmber)
                Text(content.title)
                    .font(.system(size: 20, weight: .bold, design: .rounded))
                    .multilineTextAlignment(.center)
                    .foregroundColor(LiquidGlass.textPrimary)
                Text(content.message)
                    .font(.system(size: 14))
                    .multilineTextAlignment(.center)
                    .foregroundColor(LiquidGlass.textSecondary)
                if content.offersOpenSettings {
                    Button("Open Settings") {
                        if let url = CameraAccessCoordinator.settingsURL {
                            UIApplication.shared.open(url)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(LiquidGlass.accentFill)
                    .foregroundStyle(LiquidGlass.onAccent)
                    .accessibilityIdentifier(CameraDeniedAccessibility.openSettings)
                }
                if let onClose {
                    Button("Close", action: onClose)
                        .buttonStyle(.bordered)
                        .tint(LiquidGlass.celestialTeal)
                }
            }
            .padding(28)
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier(CameraDeniedAccessibility.view)
        }
    }
}

/// Compact variant for cards: explanation plus the Settings link, nothing else.
public struct CameraDeniedInlineNotice: View {
    public let state: CameraAccessState
    public init(state: CameraAccessState) { self.state = state }

    public var body: some View {
        let content = CameraDeniedContent.make(for: state)
        VStack(alignment: .leading, spacing: 6) {
            Text(content.title)
                .font(.system(size: 12, weight: .semibold, design: .rounded))
            Text(content.message)
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            if content.offersOpenSettings {
                Button("Open Settings") {
                    if let url = CameraAccessCoordinator.settingsURL {
                        UIApplication.shared.open(url)
                    }
                }
                .font(.system(size: 12, weight: .semibold))
                .accessibilityIdentifier(CameraDeniedAccessibility.openSettings)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(CameraDeniedAccessibility.view)
    }
}

// MARK: - Scene phase wiring shared by the capture views

private struct CameraScenePhaseModifier: ViewModifier {
    @Environment(\.scenePhase) private var scenePhase
    @ObservedObject var coordinator: CameraAccessCoordinator
    let onAction: (CameraSceneAction) -> Void

    func body(content: Content) -> some View {
        content.onChange(of: scenePhase) { _, phase in
            let mapped: CameraScenePhase
            switch phase {
            case .active: mapped = .active
            case .inactive: mapped = .inactive
            case .background: mapped = .background
            @unknown default: mapped = .inactive
            }
            let action = coordinator.scenePhaseChanged(mapped)
            if action != .none { onAction(action) }
        }
    }
}

extension View {
    /// Routes scene phase changes through the coordinator (CAM-3). `onAction` runs on
    /// `.stopSession` (stop now) and `.backgroundInterruptionEnded` (foreground again).
    func cameraScenePhase(
        _ coordinator: CameraAccessCoordinator,
        onAction: @escaping (CameraSceneAction) -> Void
    ) -> some View {
        modifier(CameraScenePhaseModifier(coordinator: coordinator, onAction: onAction))
    }
}

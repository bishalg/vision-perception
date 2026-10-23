import SwiftUI
import CurvAsdGazeKit

/// Honest end state when gaze capture cannot run (SIM-2, SIM-7). There is no way
/// to start a session from here and no simulation option in any build.
public struct UnsupportedDeviceView: View {
    public let reason: DeviceBlockReason
    public var onClose: (() -> Void)?

    public init(reason: DeviceBlockReason, onClose: (() -> Void)? = nil) {
        self.reason = reason
        self.onClose = onClose
    }

    public var body: some View {
        let content = UnsupportedDeviceContent.make(for: reason)
        ZStack {
            Color.black.opacity(0.92).ignoresSafeArea()
            VStack(spacing: 16) {
                Image(systemName: content.systemImage)
                    .font(.system(size: 44, weight: .semibold))
                    .foregroundColor(LiquidGlass.warmAmber)
                Text(content.title)
                    .font(.system(size: 20, weight: .bold, design: .rounded))
                    .multilineTextAlignment(.center)
                    .foregroundColor(.white)
                Text(content.message)
                    .font(.system(size: 14))
                    .multilineTextAlignment(.center)
                    .foregroundColor(.white.opacity(0.8))
                if reason == .cameraPermissionDenied {
                    Button("Open Settings") {
                        if let url = URL(string: UIApplication.openSettingsURLString) {
                            UIApplication.shared.open(url)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                }
                if let onClose {
                    Button("Close", action: onClose)
                        .buttonStyle(.bordered)
                        .tint(.white)
                }
            }
            .padding(28)
            .accessibilityElement(children: .contain)
        }
    }
}

public extension GazeSessionController {
    /// The block to show for the current tracking state, if any.
    var blockReason: DeviceBlockReason? {
        if case .unavailable(let reason) = trackingStage { return DeviceBlockReason(reason) }
        return nil
    }
}

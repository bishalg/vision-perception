import SwiftUI

#if canImport(UIKit)
import UIKit

/// Interface-orientation lock for the capture flow (GEO-3/GEO-4 groundwork).
///
/// Held from positioning until the results screen: portrait on iPhone, and on iPad the
/// orientation in effect when the first capture view appears. Nested capture views
/// share one lock (reference counted), so the wizard and the views inside it do not
/// release each other's hold. The app delegate reads `supportedMask`.
@MainActor
final class CaptureOrientationLock {
    static let shared = CaptureOrientationLock()

    private(set) var holders = 0
    private(set) var lockedMask: UIInterfaceOrientationMask?

    /// Mask for the unlocked app, mirroring the Info.plist orientations.
    static func unlockedMask(isPad: Bool) -> UIInterfaceOrientationMask {
        isPad ? .all : .allButUpsideDown
    }

    /// Mask to hold during capture: portrait on iPhone, the current orientation on iPad.
    static func captureMask(isPad: Bool, current: UIInterfaceOrientation) -> UIInterfaceOrientationMask {
        guard isPad else { return .portrait }
        switch current {
        case .landscapeLeft: return .landscapeLeft
        case .landscapeRight: return .landscapeRight
        case .portraitUpsideDown: return .portraitUpsideDown
        default: return .portrait
        }
    }

    var supportedMask: UIInterfaceOrientationMask {
        lockedMask ?? Self.unlockedMask(isPad: UIDevice.current.userInterfaceIdiom == .pad)
    }

    func acquire() {
        holders += 1
        guard holders == 1 else { return }
        let isPad = UIDevice.current.userInterfaceIdiom == .pad
        let scene = Self.activeWindowScene
        let mask = Self.captureMask(isPad: isPad, current: scene?.interfaceOrientation ?? .portrait)
        lockedMask = mask
        apply(mask, to: scene)
    }

    func release() {
        guard holders > 0 else { return }
        holders -= 1
        guard holders == 0 else { return }
        lockedMask = nil
        apply(nil, to: Self.activeWindowScene)
    }

    private func apply(_ mask: UIInterfaceOrientationMask?, to scene: UIWindowScene?) {
        guard let scene else { return }
        scene.windows.first?.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
        if let mask {
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: mask)) { _ in }
        }
    }

    private static var activeWindowScene: UIWindowScene? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }
            ?? UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
    }
}

/// App delegate that answers the orientation query from `CaptureOrientationLock`.
final class AppOrientationDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        supportedInterfaceOrientationsFor window: UIWindow?
    ) -> UIInterfaceOrientationMask {
        MainActor.assumeIsolated { CaptureOrientationLock.shared.supportedMask }
    }
}

private struct CaptureOrientationLockModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .onAppear { CaptureOrientationLock.shared.acquire() }
            .onDisappear { CaptureOrientationLock.shared.release() }
    }
}

extension View {
    /// Locks the interface orientation while this view is on screen.
    func captureOrientationLocked() -> some View {
        modifier(CaptureOrientationLockModifier())
    }
}
#else
extension View {
    func captureOrientationLocked() -> some View { self }
}
#endif

import Foundation
#if canImport(DeclaredAgeRange)
import DeclaredAgeRange
import UIKit
#endif

// MARK: - Age Assurance Gate

/// Compile-time build-shape switch distinguishing App Store builds from internal ones.
///
/// `Release` configurations compile with `-DASD_APP_STORE_AGE_ASSURANCE` (set in
/// project.yml); internal/Debug builds do not, so the App Store age-assurance
/// request is bypassed entirely and the app behaves exactly as before.
enum AgeAssuranceGate {
    static var isAppStoreAgeAssuranceBuild: Bool {
        #if ASD_APP_STORE_AGE_ASSURANCE
        return true
        #else
        return false
        #endif
    }
}

// MARK: - Declared Age Range Service

/// Outcome of the App Store age-assurance request.
enum DeclaredAgeRangeOutcome: Equatable {
    /// Internal build — the check is bypassed by design.
    case disabledByBuildFlag
    /// Device runs an OS older than iOS 26 where the API doesn't exist.
    case osTooOld
    /// Built against an SDK without the DeclaredAgeRange framework.
    case sdkUnavailable
    /// User (or guardian) shared an age range — label like "13+" or "13–16".
    case obtained(rangeLabel: String?)
    /// User declined to share their age range.
    case declined
    case failed(String)
}

/// Thin wrapper around Apple's Declared Age Range API (iOS 26+) — the official,
/// parent-declared age-assurance path for App Store builds. Facial age estimation
/// in this app is assistive UX only; compliance goes through this API.
enum DeclaredAgeRangeService {
    /// Age gates requested from the system prompt (App Store-standard bands).
    private static let ageGates: (Int, Int?, Int?) = (13, 16, 18)

    /// Requests the App Store-declared age range when, and only when, this is an
    /// App Store build running on iOS 26+. Internal builds, older OS versions, and
    /// SDKs without the framework short-circuit without touching the API.
    @MainActor
    static func requestAgeRangeIfEnabled() async -> DeclaredAgeRangeOutcome {
        guard AgeAssuranceGate.isAppStoreAgeAssuranceBuild else {
            return .disabledByBuildFlag
        }
        #if canImport(DeclaredAgeRange)
        guard #available(iOS 26.0, *) else {
            return .osTooOld
        }
        guard let presenter = rootViewController() else {
            return .failed("No active window to present the age range prompt")
        }
        do {
            let response = try await AgeRangeService.shared.requestAgeRange(
                ageGates: ageGates.0, ageGates.1, ageGates.2,
                in: presenter
            )
            switch response {
            case .sharing(let range):
                return .obtained(rangeLabel: label(lowerBound: range.lowerBound, upperBound: range.upperBound))
            case .declinedSharing:
                return .declined
            @unknown default:
                return .failed("Unknown DeclaredAgeRange response")
            }
        } catch {
            return .failed(error.localizedDescription)
        }
        #else
        return .sdkUnavailable
        #endif
    }

    @MainActor
    private static func rootViewController() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        return scenes.first?.keyWindow?.rootViewController
            ?? scenes.flatMap { $0.windows }.first { $0.isKeyWindow }?.rootViewController
    }

    private static func label(lowerBound: Int?, upperBound: Int?) -> String {
        switch (lowerBound, upperBound) {
        case let (lower?, upper?): return "\(lower)–\(upper)"
        case let (lower?, nil): return "\(lower)+"
        case let (nil, upper?): return "≤\(upper)"
        default: return "unknown"
        }
    }
}

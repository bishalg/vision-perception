import AsdCore
import Foundation
import os

/// One trusted config bundle key shipped with the app (WP-A.5, CON-6). Several keys may be
/// trusted side by side; a bundle names its key by `keyId`, so a key can rotate without a
/// gap (ship the new key, switch the server, drop the old key in a later release).
public struct TrustedBundleKey: Sendable, Equatable, Codable {
    /// The `key_id` the backend writes (`ed25519-` + 8 bytes of SHA-256 of the public key).
    public let keyId: String
    /// The 32-byte Ed25519 public key, base64url (or standard base64).
    public let publicKey: String

    public init(keyId: String, publicKey: String) {
        self.keyId = keyId
        self.publicKey = publicKey
    }
}

/// Why a delivered bundle was refused (`asd-sync::BundleRejection`). The bundle is never used.
public enum BundleRejection: String, Error, Sendable, Equatable {
    case noTrustedKeys
    case malformedTrustedKey
    case malformed
    case unsigned
    case unknownKey
    case badSignature
    case invalidVersion
    case rollback
    case invalidSignedAt
    case expired
    case signedInFuture
    /// The signed documents do not decode as the kit's catalog / baseline types.
    case undecodableDocuments
}

/// The configuration scoring runs with: stimulus catalog, normative baselines and
/// `ScoringProfile` (WP-A.5, 02 ADR-B6).
///
/// Either the bundled default (the kit's JSON resources and the Rust core's default
/// profile) or a delivered config bundle whose Ed25519 signature, key and version policy the
/// Rust core accepted (`config_accept_bundle`; no crypto in Swift). The profile stays an
/// opaque `ScoringProfile` JSON document that only the core reads (flag thresholds,
/// low-confidence ratio); the kit decodes just the quality-gate parameters it constructs
/// `GazeQualityGate` from.
public struct ScoringConfiguration: Sendable {
    public enum Source: Sendable, Equatable {
        /// The configuration compiled into this build.
        case bundled
        /// A verified delivered bundle.
        case delivered(bundleVersion: String, keyId: String, signedAt: String)
    }

    public let source: Source
    public let catalog: StimulusCatalog
    public let baselines: NormativeBaselineSet?
    /// `ScoringProfile` (camelCase) as the core reads it.
    public let scoringProfileJSON: String
    public let featureFlags: [String: Bool]

    /// The delivered bundle version that scores with this configuration; `nil` for the bundled
    /// default. Sessions record it as `rawPayload.bundleVersion` (ING-2).
    public var bundleVersion: String? {
        if case .delivered(let version, _, _) = source { return version }
        return nil
    }

    /// The per-sample gate built from the profile's `qualityGate` (the bundled default gate
    /// when the profile cannot be read, which the core never hands out).
    public var qualityGate: GazeQualityGate {
        guard let profile = try? AsdCoreBridge.decode(CoreScoringProfile.self, from: scoringProfileJSON) else {
            return GazeQualityGate()
        }
        let g = profile.qualityGate
        return GazeQualityGate(
            minDistanceCm: g.minDistanceCm,
            maxDistanceCm: g.maxDistanceCm,
            maxYawRad: g.maxYawRad,
            maxPitchRad: g.maxPitchRad,
            minConfidence: g.minConfidence,
            blinkThreshold: g.blinkThreshold
        )
    }

    /// The configuration compiled into this build.
    public static let bundled = ScoringConfiguration(
        source: .bundled,
        catalog: StimulusCatalog.loadBundled() ?? .empty,
        baselines: NormativeBaselineLibrary.loadBundled(),
        scoringProfileJSON: AsdCoreBridge.defaultScoringProfileJSON,
        featureFlags: [:]
    )

    /// The Rust core's trust decision on a delivered bundle (signature over JCS against
    /// `trustedKeys`, no rollback below `newestAcceptedVersion`, optional maximum age of
    /// `signed_at`), then decoding of its documents into the kit types.
    public static func accept(
        bundleJSON: String,
        trustedKeys: [TrustedBundleKey],
        newestAcceptedVersion: String?,
        now: Date = Date(),
        maxAge: TimeInterval? = nil
    ) -> Result<ScoringConfiguration, BundleRejection> {
        let verdict = configAcceptBundle(
            bundleJson: bundleJSON,
            trustedKeys: trustedKeys.map { AsdTrustedKey(keyId: $0.keyId, publicKeyB64: $0.publicKey) },
            newestAcceptedVersion: newestAcceptedVersion,
            nowUnixSecs: Int64(now.timeIntervalSince1970.rounded(.down)),
            maxAgeSecs: maxAge.map { UInt64(max(0, $0)) }
        )
        switch verdict {
        case .rejected(let reason, let message):
            AsdCoreBridge.logger.error("config bundle refused: \(message, privacy: .public)")
            return .failure(BundleRejection(reason))
        case .accepted(let bundle):
            guard let catalog = StimulusCatalog.decode(Data(bundle.catalogJson.utf8)),
                  let baselines = NormativeBaselineLibrary.decode(Data(bundle.baselinesJson.utf8)) else {
                AsdCoreBridge.logger.error("config bundle \(bundle.bundleVersion, privacy: .public) documents do not decode")
                return .failure(.undecodableDocuments)
            }
            return .success(ScoringConfiguration(
                source: .delivered(bundleVersion: bundle.bundleVersion, keyId: bundle.keyId, signedAt: bundle.signedAt),
                catalog: catalog,
                baselines: baselines,
                scoringProfileJSON: bundle.scoringProfileJson,
                featureFlags: bundle.featureFlags
            ))
        }
    }

    // MARK: Active configuration

    private static let storage = OSAllocatedUnfairLock(initialState: ScoringConfiguration.bundled)

    /// The configuration in force. Install a delivered one at launch, before any session
    /// starts, so a session never sees two configurations.
    public static var active: ScoringConfiguration {
        storage.withLock { $0 }
    }

    /// Makes `configuration` the active one (`StimulusCatalog.current` follows it).
    public static func install(_ configuration: ScoringConfiguration) {
        storage.withLock { $0 = configuration }
    }
}

extension BundleRejection {
    init(_ reason: AsdBundleRejection) {
        switch reason {
        case .noTrustedKeys: self = .noTrustedKeys
        case .malformedTrustedKey: self = .malformedTrustedKey
        case .malformed: self = .malformed
        case .unsigned: self = .unsigned
        case .unknownKey: self = .unknownKey
        case .badSignature: self = .badSignature
        case .invalidVersion: self = .invalidVersion
        case .rollback: self = .rollback
        case .invalidSignedAt: self = .invalidSignedAt
        case .expired: self = .expired
        case .signedInFuture: self = .signedInFuture
        }
    }
}

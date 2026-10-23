import Foundation
import CryptoKit
import Security

/// Manages on-device AES-GCM encrypted storage and export serialization for gaze sessions.
public struct SessionVaultManager: Sendable {
    public let vaultDirectory: URL
    private let symmetricKey: SymmetricKey

    private static let keychainService = "org.curv.asd.vault"
    private static let keychainAccount = "vault-key-v1"

    /// True when the vault key is durable across launches (explicit key or Keychain).
    /// False means the Keychain was unavailable and an ephemeral key is in use.
    public var isKeyPersisted: Bool {
        keyDataWasPersisted
    }
    private let keyDataWasPersisted: Bool

    /// Initializes vault manager with a dedicated sandbox directory and encryption key.
    ///
    /// Key resolution order:
    /// 1. Explicit `keyData` (tests / deterministic round-trips)
    /// 2. Keychain-persisted vault key — loaded or created on first use, so sessions
    ///    written in a previous launch remain decryptable (the original implementation
    ///    generated a fresh key per instance, orphaning every prior session)
    /// 3. Ephemeral random key — only when the Keychain is unavailable
    public init(vaultDirectory: URL? = nil, keyData: Data? = nil) {
        if let dir = vaultDirectory {
            self.vaultDirectory = dir
        } else {
            let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
                ?? FileManager.default.temporaryDirectory
            self.vaultDirectory = appSupport.appendingPathComponent("org.curv.asd.vault", isDirectory: true)
        }

        try? FileManager.default.createDirectory(at: self.vaultDirectory, withIntermediateDirectories: true)
        Self.excludeFromBackup(self.vaultDirectory)

        if let keyData = keyData, keyData.count == 32 {
            self.symmetricKey = SymmetricKey(data: keyData)
            self.keyDataWasPersisted = true
        } else {
            let resolved = Self.loadOrCreatePersistentKey()
            self.symmetricKey = resolved.key
            self.keyDataWasPersisted = resolved.persisted
        }
    }

    /// Loads the vault key from the Keychain, creating and storing a new 256-bit key
    /// when none exists. Falls back to an ephemeral key (matching the old behavior)
    /// if the Keychain cannot be accessed.
    private static func loadOrCreatePersistentKey() -> (key: SymmetricKey, persisted: Bool) {
        let baseQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: keychainAccount
        ]

        var readQuery = baseQuery
        readQuery[kSecReturnData as String] = true
        readQuery[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let readStatus = SecItemCopyMatching(readQuery as CFDictionary, &item)
        if readStatus == errSecSuccess, let data = item as? Data, data.count == 32 {
            return (SymmetricKey(data: data), true)
        }

        let key = SymmetricKey(size: .bits256)
        let keyData = key.withUnsafeBytes { Data($0) }

        SecItemDelete(baseQuery as CFDictionary) // clear stale/malformed item before rewrite
        var addQuery = baseQuery
        addQuery[kSecValueData as String] = keyData
        addQuery[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let addStatus = SecItemAdd(addQuery as CFDictionary, nil)
        if addStatus != errSecSuccess {
            NSLog("[SessionVault] Keychain unavailable (status \(addStatus)) — using an ephemeral vault key; previously saved sessions will not decrypt.")
        }
        return (key, addStatus == errSecSuccess)
    }

    /// Options for every vault file write (CNS-3): atomic, and unreadable while the
    /// device is locked.
    public static let fileWritingOptions: Data.WritingOptions = [.atomic, .completeFileProtection]

    /// Filename suffix; simulated sessions are kept apart for developer debugging (SIM-5).
    static func fileSuffix(for source: CaptureSource) -> String {
        switch source {
        case .trueDepth: return ".jsonl.enc"
        case .simulated: return ".sim.jsonl.enc"
        }
    }

    /// Marks the vault directory as excluded from iCloud/iTunes backups (CNS-3).
    private static func excludeFromBackup(_ directory: URL) {
        var url = directory
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        do {
            try url.setResourceValues(values)
        } catch {
            NSLog("[SessionVault] Could not exclude vault directory from backup: \(error.localizedDescription)")
        }
    }

    /// Encrypts and writes session header, frames and report to disk as a single post-session batch.
    public func saveSession(
        header: SessionHeader,
        frames: [GazeFrame],
        report: GazeSessionReportSummary,
        perVideoScore: PerVideoSessionScore? = nil,
        collectionMetadata: NormCollectionMetadata? = nil
    ) throws -> URL {
        try writeSession(
            header: header,
            frames: frames,
            report: report,
            classificationLine: nil,
            perVideoScore: perVideoScore,
            collectionMetadata: collectionMetadata
        )
    }

    #if ASD_RESEARCH_CLASSIFIER
    /// Research builds only (CLM-3): also writes a classifier result line.
    public func saveSession(
        header: SessionHeader,
        frames: [GazeFrame],
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult?,
        perVideoScore: PerVideoSessionScore? = nil,
        collectionMetadata: NormCollectionMetadata? = nil
    ) throws -> URL {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let line = classification
            .flatMap { try? encoder.encode($0) }
            .flatMap { String(data: $0, encoding: .utf8) }
        return try writeSession(
            header: header,
            frames: frames,
            report: report,
            classificationLine: line,
            perVideoScore: perVideoScore,
            collectionMetadata: collectionMetadata
        )
    }
    #endif

    private func writeSession(
        header: SessionHeader,
        frames: [GazeFrame],
        report: GazeSessionReportSummary,
        classificationLine: String?,
        perVideoScore: PerVideoSessionScore?,
        collectionMetadata: NormCollectionMetadata?
    ) throws -> URL {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]

        var lines: [String] = []
        if let headerData = try? encoder.encode(header), let headerStr = String(data: headerData, encoding: .utf8) {
            lines.append(headerStr)
        }
        for frame in frames {
            if let frameData = try? encoder.encode(frame), let frameStr = String(data: frameData, encoding: .utf8) {
                lines.append(frameStr)
            }
        }
        if let reportData = try? encoder.encode(report), let reportStr = String(data: reportData, encoding: .utf8) {
            lines.append(reportStr)
        }
        if let clStr = classificationLine {
            lines.append(clStr)
        }
        if let pv = perVideoScore,
           let pvData = try? encoder.encode(pv),
           let pvStr = String(data: pvData, encoding: .utf8) {
            lines.append("{\"type\":\"per_video_score\",\"score\":\(pvStr)}")
        }
        if let collection = collectionMetadata,
           let cData = try? encoder.encode(collection),
           let cStr = String(data: cData, encoding: .utf8) {
            lines.append("{\"type\":\"norm_collection_meta\",\"collection\":\(cStr)}")
        }

        let plainText = lines.joined(separator: "\n")
        let plainData = Data(plainText.utf8)

        // AES-GCM-256 sealed box
        let sealedBox = try AES.GCM.seal(plainData, using: symmetricKey)
        guard let combined = sealedBox.combined else {
            throw NSError(domain: "SessionVault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Failed to assemble sealed box"])
        }

        let filename = "session_\(header.sessionId)_\(header.sessionStartUtc.prefix(10))\(Self.fileSuffix(for: header.captureSource))"
        let fileURL = vaultDirectory.appendingPathComponent(filename)
        try combined.write(to: fileURL, options: Self.fileWritingOptions)
        return fileURL
    }

    /// Asynchronously batches, encrypts, and writes session to disk off the main thread.
    public func saveSessionBatchAsync(
        header: SessionHeader,
        frames: [GazeFrame],
        report: GazeSessionReportSummary,
        perVideoScore: PerVideoSessionScore? = nil,
        collectionMetadata: NormCollectionMetadata? = nil
    ) async throws -> URL {
        try await runOffMain {
            try self.saveSession(
                header: header,
                frames: frames,
                report: report,
                perVideoScore: perVideoScore,
                collectionMetadata: collectionMetadata
            )
        }
    }

    #if ASD_RESEARCH_CLASSIFIER
    /// Research builds only (CLM-3): async variant that also writes a classifier line.
    public func saveSessionBatchAsync(
        header: SessionHeader,
        frames: [GazeFrame],
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult?,
        perVideoScore: PerVideoSessionScore? = nil,
        collectionMetadata: NormCollectionMetadata? = nil
    ) async throws -> URL {
        try await runOffMain {
            try self.saveSession(
                header: header,
                frames: frames,
                report: report,
                classification: classification,
                perVideoScore: perVideoScore,
                collectionMetadata: collectionMetadata
            )
        }
    }
    #endif

    private func runOffMain(_ work: @escaping @Sendable () throws -> URL) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    continuation.resume(returning: try work())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    /// Reads and decrypts a session file from the vault.
    public func readSessionPlainText(from fileURL: URL) throws -> String {
        let encryptedData = try Data(contentsOf: fileURL)
        let sealedBox = try AES.GCM.SealedBox(combined: encryptedData)
        let decryptedData = try AES.GCM.open(sealedBox, using: symmetricKey)
        guard let text = String(data: decryptedData, encoding: .utf8) else {
            throw NSError(domain: "SessionVault", code: 2, userInfo: [NSLocalizedDescriptionKey: "Malformed UTF-8 string"])
        }
        return text
    }

    #if ASD_RESEARCH_CLASSIFIER
    /// Research builds only (CLM-3): ResearchKit JSON with a `classification` block.
    public static func makeResearchKitJson(
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult?,
        perVideoScore: PerVideoSessionScore? = nil,
        collection: [String: Any]? = nil
    ) -> [String: Any] {
        var base = makeResearchKitJson(report: report, perVideoScore: perVideoScore, collection: collection)
        if let cl = classification {
            base["classification"] = [
                "riskScore": cl.riskScore,
                "confidence": cl.confidence,
                "riskStratification": cl.riskStratification.rawValue,
                "primaryContributingFeature": cl.primaryContributingFeature,
                "executionDevice": cl.executionDevice.rawValue,
                "inferenceLatencyMs": cl.inferenceLatencyMs
            ]
        }
        return base
    }
    #endif

    /// Exports the session report conforming to the Apple ResearchKit ORKActiveStepResult schema.
    /// Absent metrics are omitted (PLC-3); nothing substitutes a default.
    public static func makeResearchKitJson(
        report: GazeSessionReportSummary,
        perVideoScore: PerVideoSessionScore? = nil,
        collection: [String: Any]? = nil
    ) -> [String: Any] {
        var biomarkers: [String: Any] = [
            "socialPreferenceScore": report.socialPreferenceScore,
            "socialSmileIntensity": report.socialSmileIntensityMean,
            "smileReactivityIndex": report.smileReactivityIndex,
            "flatAffectScore": report.flatAffectScore,
            "surpriseScoreMean": report.surpriseScoreMean
        ]
        if let value = report.eyeDwellRatio { biomarkers["eyeDwellRatio"] = value }
        if let value = report.mouthDwellRatio { biomarkers["mouthDwellRatio"] = value }
        if let value = report.firstEyeLatencyMs { biomarkers["firstEyeLatencyMs"] = value }

        var base: [String: Any] = [
            "identifier": "ORKActiveStepResult",
            "type": "social_attention_gaze_result",
            "sessionId": report.sessionId,
            "durationSec": report.durationSec,
            "isValid": report.validRatio >= 0.60,
            "validSampleRatio": report.validRatio,
            "qualityVerdict": report.qualityVerdict.rawValue,
            "captureSource": report.captureSource.rawValue,
            "biomarkers": biomarkers
        ]
        if let collection = collection {
            base["collection"] = collection
        }
        if let pv = perVideoScore {
            base["per_video"] = [
                "ageBracketId": pv.ageBracketId,
                "ageMonths": pv.ageMonths,
                "compositeScore": pv.compositeScore,
                "baselineStatus": pv.baselineStatus,
                "baselineSchemaVersion": pv.baselineSchemaVersion,
                "scoredAtUtc": pv.scoredAtUtc,
                "videos": pv.videos.map { video in
                    [
                        "stimulusId": video.stimulusId,
                        "title": video.title,
                        "subscore": video.subscore,
                        "hasBaseline": video.hasBaseline,
                        "lowConfidence": video.lowConfidence,
                        "validSampleRatio": video.validSampleRatio,
                        "rows": video.rows.map { row in
                            [
                                "segmentId": row.segmentId,
                                "biomarker": row.biomarker,
                                // Nil optionals must become explicit JSON nulls — storing
                                // Optional.none in [String: Any] makes JSONSerialization throw,
                                // silently degrading the whole share export to a stub.
                                "targetAOI": row.targetAOI.map { $0 as Any } ?? NSNull(),
                                "observed": row.observed,
                                "expected": row.expected.map { $0 as Any } ?? NSNull(),
                                "status": row.status.rawValue,
                                "score": row.score,
                                "imputed": row.imputed,
                                "citation": row.citation,
                                "source": row.source
                            ] as [String: Any]
                        }
                    ] as [String: Any]
                }
            ]
        }
        return base
    }

    #if ASD_RESEARCH_CLASSIFIER
    /// Research builds only (CLM-3): CareKit JSON with classifier outcome fields.
    public static func makeCareKitJson(
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult?
    ) -> [String: Any] {
        var outcome: [String: Any] = [:]
        if let cl = classification {
            outcome["riskScore"] = cl.riskScore
            outcome["riskStratification"] = cl.riskStratification.rawValue
            outcome["confidence"] = cl.confidence
            outcome["executionDevice"] = cl.executionDevice.rawValue
        }
        return careKitJson(report: report, extraOutcome: outcome)
    }
    #endif

    /// Exports the session report conforming to the Apple CareKit OCKCarePlanActivity schema.
    public static func makeCareKitJson(report: GazeSessionReportSummary) -> [String: Any] {
        careKitJson(report: report, extraOutcome: [:])
    }

    private static func careKitJson(report: GazeSessionReportSummary, extraOutcome: [String: Any]) -> [String: Any] {
        var outcome: [String: Any] = [
            "socialPreferenceScore": report.socialPreferenceScore,
            "qualityVerdict": report.qualityVerdict.rawValue,
            "captureSource": report.captureSource.rawValue,
            "timestamp": ISO8601DateFormatter().string(from: Date())
        ]
        outcome.merge(extraOutcome) { _, new in new }
        return [
            "identifier": "OCKCarePlanActivity",
            "title": "Pediatric Visual Attention Screening",
            "sessionId": report.sessionId,
            "status": "completed",
            "outcome": outcome
        ]
    }
}

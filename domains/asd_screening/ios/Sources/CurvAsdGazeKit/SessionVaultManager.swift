import Foundation
import CryptoKit

/// Manages on-device AES-GCM encrypted storage and export serialization for gaze sessions.
public struct SessionVaultManager: Sendable {
    public let vaultDirectory: URL
    private let symmetricKey: SymmetricKey

    /// Initializes vault manager with a dedicated sandbox directory and encryption key.
    public init(vaultDirectory: URL? = nil, keyData: Data? = nil) {
        if let dir = vaultDirectory {
            self.vaultDirectory = dir
        } else {
            let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
                ?? FileManager.default.temporaryDirectory
            self.vaultDirectory = appSupport.appendingPathComponent("org.curv.asd.vault", isDirectory: true)
        }

        try? FileManager.default.createDirectory(at: self.vaultDirectory, withIntermediateDirectories: true)

        if let keyData = keyData, keyData.count == 32 {
            self.symmetricKey = SymmetricKey(data: keyData)
        } else {
            // Default 256-bit key for local vault
            self.symmetricKey = SymmetricKey(size: .bits256)
        }
    }

    /// Encrypts and writes session header, frames, report, and optional classification to disk as a single post-session batch.
    public func saveSession(
        header: SessionHeader,
        frames: [GazeFrame],
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult? = nil,
        perVideoScore: PerVideoSessionScore? = nil,
        collectionMetadata: NormCollectionMetadata? = nil
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
        if let cl = classification, let clData = try? encoder.encode(cl), let clStr = String(data: clData, encoding: .utf8) {
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

        let filename = "session_\(header.sessionId)_\(header.sessionStartUtc.prefix(10)).jsonl.enc"
        let fileURL = vaultDirectory.appendingPathComponent(filename)
        try combined.write(to: fileURL, options: .atomic)
        return fileURL
    }

    /// Asynchronously batches, encrypts, and writes session to disk off the main thread.
    public func saveSessionBatchAsync(
        header: SessionHeader,
        frames: [GazeFrame],
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult? = nil,
        perVideoScore: PerVideoSessionScore? = nil,
        collectionMetadata: NormCollectionMetadata? = nil
    ) async throws -> URL {
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let url = try self.saveSession(
                        header: header,
                        frames: frames,
                        report: report,
                        classification: classification,
                        perVideoScore: perVideoScore,
                        collectionMetadata: collectionMetadata
                    )
                    continuation.resume(returning: url)
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

    /// Exports the session report conforming to the Apple ResearchKit ORKActiveStepResult schema.
    public static func makeResearchKitJson(
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult? = nil,
        perVideoScore: PerVideoSessionScore? = nil,
        collection: [String: Any]? = nil
    ) -> [String: Any] {
        var base: [String: Any] = [
            "identifier": "ORKActiveStepResult",
            "type": "social_attention_gaze_result",
            "sessionId": report.sessionId,
            "durationSec": report.durationSec,
            "isValid": report.validRatio >= 0.60,
            "validSampleRatio": report.validRatio,
            "qualityVerdict": report.qualityVerdict.rawValue,
            "biomarkers": [
                "socialPreferenceScore": report.socialPreferenceScore,
                "eyeDwellRatio": report.eyeDwellRatio,
                "mouthDwellRatio": report.mouthDwellRatio,
                "firstEyeLatencyMs": report.firstEyeLatencyMs,
                "socialSmileIntensity": report.socialSmileIntensityMean,
                "smileReactivityIndex": report.smileReactivityIndex,
                "flatAffectScore": report.flatAffectScore,
                "surpriseScoreMean": report.surpriseScoreMean
            ]
        ]
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
                                "targetAOI": row.targetAOI as Any?,
                                "observed": row.observed,
                                "expected": row.expected as Any?,
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

    /// Exports the session report conforming to the Apple CareKit OCKCarePlanActivity schema.
    public static func makeCareKitJson(
        report: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult? = nil
    ) -> [String: Any] {
        var outcome: [String: Any] = [
            "socialPreferenceScore": report.socialPreferenceScore,
            "qualityVerdict": report.qualityVerdict.rawValue,
            "timestamp": ISO8601DateFormatter().string(from: Date())
        ]
        if let cl = classification {
            outcome["riskScore"] = cl.riskScore
            outcome["riskStratification"] = cl.riskStratification.rawValue
            outcome["confidence"] = cl.confidence
            outcome["executionDevice"] = cl.executionDevice.rawValue
        }
        return [
            "identifier": "OCKCarePlanActivity",
            "title": "Pediatric Visual Attention Screening",
            "sessionId": report.sessionId,
            "status": "completed",
            "outcome": outcome
        ]
    }
}

import CurvAsdGazeKit
import Foundation
import os

/// The single place that knows every on-device store holding observation data
/// about a child (CNS-6, ADR-0002 decision 6).
///
/// `deleteAll(includeCarePlans:)` empties the vault, session history, research
/// upload queues, the `BlendShapeML` directory if a data-export build left one,
/// the `BenchLogs` directory if a bench-capture build left one, the calibration profile, M-CHAT results, study enrollment
/// and the child profile's participant token (REC-11: later sessions are a new participant). CareKit care
/// plans are the parent's own notes, not telemetry: they are deleted only when
/// the caller passes `includeCarePlans: true` after a separate confirmation.
///
/// S-24: the store build has no care plans and offers one confirmation that removes
/// everything: `deleteEverything()` also removes the guardian consent record, every
/// opt-in and the child profile, so the app starts over as on first launch.
@MainActor
public final class DataDeletionService {
    /// Items that could not be deleted, named for the user-facing error.
    public struct Report: Equatable, Sendable {
        public var failures: [String] = []
        public var succeeded: Bool { failures.isEmpty }
    }

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "DataDeletionService")

    nonisolated static let blendShapeFolder = "BlendShapeML"
    nonisolated static let benchLogFolder = "BenchLogs"

    private let fileManager: FileManager
    private let vaultDirectory: URL
    private let blendShapeDirectory: URL
    private let benchLogDirectory: URL
    private let historyStore: GazeSessionHistoryStore
    private let researchStore: ResearchKitStudyStore
    private let purgeQueues: @MainActor () -> Void
    private let resetCalibration: @MainActor () -> Void
    private let forgetParticipantToken: @MainActor () -> Void
    private let deleteCarePlansAction: @MainActor () -> Void
    private let resetConsentAndSettings: @MainActor () -> Void

    public init(
        vaultDirectory: URL,
        blendShapeDirectory: URL,
        benchLogDirectory: URL,
        historyStore: GazeSessionHistoryStore,
        researchStore: ResearchKitStudyStore,
        purgeQueues: @escaping @MainActor () -> Void,
        resetCalibration: @escaping @MainActor () -> Void,
        forgetParticipantToken: @escaping @MainActor () -> Void,
        deleteCarePlans: @escaping @MainActor () -> Void,
        resetConsentAndSettings: @escaping @MainActor () -> Void = {},
        fileManager: FileManager = .default
    ) {
        self.vaultDirectory = vaultDirectory
        self.blendShapeDirectory = blendShapeDirectory
        self.benchLogDirectory = benchLogDirectory
        self.historyStore = historyStore
        self.researchStore = researchStore
        self.purgeQueues = purgeQueues
        self.resetCalibration = resetCalibration
        self.forgetParticipantToken = forgetParticipantToken
        self.deleteCarePlansAction = deleteCarePlans
        self.resetConsentAndSettings = resetConsentAndSettings
        self.fileManager = fileManager
    }

    /// Production wiring over the shared stores.
    public static func live() -> DataDeletionService {
        DataDeletionService(
            vaultDirectory: SessionVaultManager().vaultDirectory,
            blendShapeDirectory: defaultBlendShapeDirectory(),
            benchLogDirectory: defaultBenchLogDirectory(),
            historyStore: .shared,
            researchStore: .shared,
            purgeQueues: { ResearchKitSyncService.purgeQueuedUploads() },
            resetCalibration: {
                BoundCalibrationStore.reset()
                NotificationCenter.default.post(
                    name: GazeSessionController.didUpdateCalibrationNotification,
                    object: nil
                )
            },
            forgetParticipantToken: { ChildProfileStore.shared.forgetParticipantToken() },
            deleteCarePlans: { CarePlanStore.shared.deleteAllCarePlanData() },
            resetConsentAndSettings: {
                ConsentRecordStore.shared.revoke()
                FeatureOptIns.shared.resetAll()
                ChildProfileStore.shared.reset()
            }
        )
    }

    /// Where the Debug-only blend-shape logger writes (`ASD_ML_DATA_EXPORT`).
    /// Known here unconditionally so deletion also cleans up after such a build.
    public nonisolated static func defaultBlendShapeDirectory(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.temporaryDirectory
        return base.appendingPathComponent(blendShapeFolder, isDirectory: true)
    }

    /// Where the Debug-only bench capture (`ASD_BENCH_CAPTURE`, WP-G.8) writes its raw
    /// tracking logs. Known here unconditionally so deletion also cleans up after such a build.
    public nonisolated static func defaultBenchLogDirectory(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .documentDirectory, in: .userDomainMask).first
            ?? fileManager.temporaryDirectory
        return base.appendingPathComponent(benchLogFolder, isDirectory: true)
    }

    /// Deletes every observation store. Each store is attempted even if an
    /// earlier one failed; failures are reported, not thrown.
    @discardableResult
    public func deleteAll(includeCarePlans: Bool) -> Report {
        var report = Report()

        if !removeContents(of: vaultDirectory) {
            report.failures.append("encrypted session storage")
        }
        historyStore.clear()
        researchStore.deleteAllStudyData()
        purgeQueues()
        if !removeItemIfPresent(blendShapeDirectory) {
            report.failures.append("blend-shape export folder")
        }
        if !removeItemIfPresent(benchLogDirectory) {
            report.failures.append("bench logs")
        }
        resetCalibration()
        forgetParticipantToken()

        if includeCarePlans {
            deleteCarePlans()
        }

        if report.succeeded {
            Self.logger.info("Deleted all on-device observation data (care plans included: \(includeCarePlans))")
        } else {
            Self.logger.error("Observation data deletion incomplete: \(report.failures.joined(separator: ", "), privacy: .public)")
        }
        return report
    }

    /// Store-build delete-all (S-24), behind a single confirmation: every observation store
    /// (as `deleteAll(includeCarePlans: false)`; the store build has no care plans), then the
    /// guardian consent record, every opt-in and the child profile. The consent and settings
    /// reset runs even when a file could not be removed, so nothing keeps the old consent.
    @discardableResult
    public func deleteEverything() -> Report {
        let report = deleteAll(includeCarePlans: false)
        resetConsentAndSettings()
        Self.logger.info("Reset consent record, opt-ins and child profile")
        return report
    }

    /// Deletes the caregiver's care plans. Only after its own confirmation.
    public func deleteCarePlans() {
        deleteCarePlansAction()
        Self.logger.info("Deleted caregiver care plans")
    }

    // MARK: - File helpers

    /// Empties `dir` but keeps it, so its protection and backup-exclusion
    /// attributes survive for the next write. A missing directory is empty.
    private func removeContents(of dir: URL) -> Bool {
        guard fileManager.fileExists(atPath: dir.path) else { return true }
        let items: [URL]
        do {
            items = try fileManager.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
        } catch {
            Self.logger.error("Cannot list \(dir.lastPathComponent, privacy: .public): \(error.localizedDescription, privacy: .public)")
            return false
        }
        var ok = true
        for item in items where !removeItemIfPresent(item) {
            ok = false
        }
        return ok
    }

    private func removeItemIfPresent(_ url: URL) -> Bool {
        guard fileManager.fileExists(atPath: url.path) else { return true }
        do {
            try fileManager.removeItem(at: url)
            return true
        } catch {
            Self.logger.error("Cannot delete \(url.lastPathComponent, privacy: .public): \(error.localizedDescription, privacy: .public)")
            return false
        }
    }
}

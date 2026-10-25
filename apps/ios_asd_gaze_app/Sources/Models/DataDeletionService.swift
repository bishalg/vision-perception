import CurvAsdGazeKit
import Foundation
import os

/// The single place that knows every on-device store holding observation data
/// about a child (CNS-6, ADR-0002 decision 6).
///
/// `deleteAll(includeCarePlans:)` empties the vault, session history, research
/// upload queues, the `BlendShapeML` directory if a data-export build left one,
/// the calibration profile, M-CHAT results and study enrollment. CareKit care
/// plans are the parent's own notes, not telemetry: they are deleted only when
/// the caller passes `includeCarePlans: true` after a separate confirmation.
@MainActor
public final class DataDeletionService {
    /// Items that could not be deleted, named for the user-facing error.
    public struct Report: Equatable, Sendable {
        public var failures: [String] = []
        public var succeeded: Bool { failures.isEmpty }
    }

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "DataDeletionService")

    static let blendShapeFolder = "BlendShapeML"

    private let fileManager: FileManager
    private let vaultDirectory: URL
    private let blendShapeDirectory: URL
    private let historyStore: GazeSessionHistoryStore
    private let researchStore: ResearchKitStudyStore
    private let purgeQueues: @MainActor () -> Void
    private let resetCalibration: @MainActor () -> Void
    private let deleteCarePlansAction: @MainActor () -> Void

    public init(
        vaultDirectory: URL,
        blendShapeDirectory: URL,
        historyStore: GazeSessionHistoryStore,
        researchStore: ResearchKitStudyStore,
        purgeQueues: @escaping @MainActor () -> Void,
        resetCalibration: @escaping @MainActor () -> Void,
        deleteCarePlans: @escaping @MainActor () -> Void,
        fileManager: FileManager = .default
    ) {
        self.vaultDirectory = vaultDirectory
        self.blendShapeDirectory = blendShapeDirectory
        self.historyStore = historyStore
        self.researchStore = researchStore
        self.purgeQueues = purgeQueues
        self.resetCalibration = resetCalibration
        self.deleteCarePlansAction = deleteCarePlans
        self.fileManager = fileManager
    }

    /// Production wiring over the shared stores.
    public static func live() -> DataDeletionService {
        DataDeletionService(
            vaultDirectory: SessionVaultManager().vaultDirectory,
            blendShapeDirectory: defaultBlendShapeDirectory(),
            historyStore: .shared,
            researchStore: .shared,
            purgeQueues: { ResearchKitSyncService.purgeQueuedUploads() },
            resetCalibration: {
                GazeCalibrationStore.reset()
                NotificationCenter.default.post(
                    name: GazeSessionController.didUpdateCalibrationNotification,
                    object: nil
                )
            },
            deleteCarePlans: { CarePlanStore.shared.deleteAllCarePlanData() }
        )
    }

    /// Where the Debug-only blend-shape logger writes (`ASD_ML_DATA_EXPORT`).
    /// Known here unconditionally so deletion also cleans up after such a build.
    public nonisolated static func defaultBlendShapeDirectory(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.temporaryDirectory
        return base.appendingPathComponent(blendShapeFolder, isDirectory: true)
    }

    /// Deletes every observation store. Each store is attempted even if an
    /// earlier one failed; failures are reported, not thrown.
    @discardableResult
    public func deleteAll(includeCarePlans: Bool) -> Report {
        var report = Report()

        if !removeContents(of: vaultDirectory) {
            report.failures.append("session vault")
        }
        historyStore.clear()
        researchStore.deleteAllStudyData()
        purgeQueues()
        if !removeItemIfPresent(blendShapeDirectory) {
            report.failures.append("blend-shape export folder")
        }
        resetCalibration()

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

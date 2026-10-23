import Foundation
import Combine
import os

/// Manages dynamic downloading, local caching, and asset resolution of video stimuli
/// using Apple On-Demand Resources (ODR / `NSBundleResourceRequest`).
///
/// Designed for offline-first clinical resilience:
/// 1. Prioritizes pre-bundled local files.
/// 2. If an ODR pack is requested, checks local cache before initiating App Store CDN fetch.
/// 3. Falls back gracefully to bundled baseline assets or procedural engine if offline.
/// Staged progress for the onboarding download splash: the splash renders
/// these as a checklist so caregivers can see gradual Apple-CDN progress.
public enum StimulusPackStage: String, Sendable, Equatable {
    case idle
    case checkingLocal
    case downloading
    case verifying
    case ready
}

/// One playlist clip on the download checklist.
public struct StimulusClipTransfer: Identifiable, Equatable, Sendable {
    public enum Phase: Equatable, Sendable {
        case onDevice
        case waiting
        case downloading(Double)
        case missing
    }

    public let id: String
    public let title: String
    public var phase: Phase

    public var statusLabel: String {
        switch phase {
        case .onDevice: return "On device"
        case .waiting: return "Waiting"
        case .downloading(let fraction): return "\(Int(fraction * 100))%"
        case .missing: return "Not on device"
        }
    }
}

enum StimulusCDN {
    /// Optional HTTP root, Info.plist key `StimulusCDNBaseURL`, no trailing path.
    /// Each clip is fetched as `{base}/{videoName}.mp4`.
    static var baseURL: URL? {
        guard let raw = Bundle.main.object(forInfoDictionaryKey: "StimulusCDNBaseURL") as? String else {
            return nil
        }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let url = URL(string: trimmed) else { return nil }
        return url
    }
}

@MainActor
public final class OnDemandStimulusManager: ObservableObject {
    public static let shared = OnDemandStimulusManager()

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "OnDemandStimulusManager")

    @Published public private(set) var downloadProgress: Double = 0.0
    @Published public private(set) var isDownloading: Bool = false
    @Published public private(set) var activeTag: String? = nil
    @Published public private(set) var lastError: String? = nil
    /// Live Apple-CDN transfer counters (bytes) for the splash MB readout.
    @Published public private(set) var downloadedBytes: Int64 = 0
    @Published public private(set) var totalBytes: Int64 = 0
    /// True when the CDN was unreachable and bundled/procedural assets stand in.
    @Published public private(set) var usedOfflineFallback: Bool = false
    @Published public private(set) var stage: StimulusPackStage = .idle
    @Published public private(set) var clips: [StimulusClipTransfer] = []

    private var currentRequest: NSBundleResourceRequest?
    private var progressObserver: AnyCancellable?

    private init() {}

    // MARK: - Tag Resolution

    /// Maps child age in months and optional locale code to the appropriate On-Demand Resource tag.
    public func resolveTag(ageMonths: Int, localeCode: String = "en") -> String {
        let ageBracket: String
        switch ageMonths {
        case ..<24:
            ageBracket = "age_12_24"
        case 24..<36:
            ageBracket = "age_24_36"
        default:
            ageBracket = "age_36_60"
        }

        let normalizedLang = localeCode.lowercased().prefix(2)
        switch normalizedLang {
        case "hi":
            return "pack_\(ageBracket)_hi"
        case "ne":
            return "pack_\(ageBracket)_ne"
        case "zh":
            return "pack_\(ageBracket)_zh"
        default:
            return "pack_\(ageBracket)_en"
        }
    }

    // MARK: - Resource Preloading

    /// Preloads the stimulus pack for the designated age and locale.
    /// Returns `true` if resources are verified and available locally on disk.
    @discardableResult
    public func preloadStimulusPack(ageMonths: Int, localeCode: String = "en") async -> Bool {
        let tag = resolveTag(ageMonths: ageMonths, localeCode: localeCode)
        return await preloadPack(tag: tag)
    }

    /// Preloads resources matching a specific ODR tag.
    @discardableResult
    public func preloadPack(tag: String) async -> Bool {
        Self.logger.info("Preloading ODR stimulus pack: \(tag, privacy: .public)")

        // If already accessing this tag, return immediately
        if activeTag == tag, currentRequest != nil {
            return true
        }

        // Release any existing held request
        releaseCurrentPack()

        let request = NSBundleResourceRequest(tags: [tag])
        request.loadingPriority = NSBundleResourceRequestLoadingPriorityUrgent
        self.currentRequest = request
        self.isDownloading = true
        self.downloadProgress = 0.0
        self.lastError = nil
        self.usedOfflineFallback = false
        self.downloadedBytes = 0
        self.totalBytes = 0
        self.stage = .checkingLocal

        // Observe fraction completed + byte counters for the splash UI.
        progressObserver = request.progress
            .publisher(for: \.fractionCompleted)
            .receive(on: DispatchQueue.main)
            .sink { [weak self] fraction in
                self?.downloadProgress = fraction
                if let progress = self?.currentRequest?.progress {
                    self?.downloadedBytes = progress.completedUnitCount
                    self?.totalBytes = progress.totalUnitCount
                    if fraction > 0, self?.stage == .checkingLocal {
                        self?.stage = .downloading
                    }
                }
            }

        let playlist = StimulusPlaylistBattery.shippedClips
        publishInventory(playlist)

        // Every shipped clip is already on device. Nothing to fetch.
        if clips.allSatisfy({ $0.phase == .onDevice }) {
            Self.logger.info("All \(playlist.count) stimulus clips are already on device.")
            finishResolved(tag: tag, offline: false)
            return true
        }

        if let base = StimulusCDN.baseURL {
            self.stage = .downloading
            for item in playlist where resolveVideoURL(name: item.videoName) == nil {
                await downloadClip(item, from: base)
            }
            publishInventory(playlist)
            let offline = clips.contains { $0.phase == .missing }
            finishResolved(tag: tag, offline: offline)
            return !offline
        }

        // 2. Check if resources are already cached locally via ODR
        let alreadyAvailable = await request.conditionallyBeginAccessingResources()
        if alreadyAvailable {
            Self.logger.info("ODR pack \(tag, privacy: .public) is already cached locally.")
            publishInventory(playlist, unresolvedAsMissing: true)
            let offline = clips.contains { $0.phase != .onDevice }
            finishResolved(tag: tag, offline: offline)
            return !offline
        }

        // 3. Fetch from Apple CDN (with seamless graceful fallback)
        self.stage = .downloading
        do {
            try await request.beginAccessingResources()
            Self.logger.info("Successfully fetched ODR pack: \(tag, privacy: .public)")
            publishInventory(playlist)
            let offline = clips.contains { $0.phase != .onDevice }
            finishResolved(tag: tag, offline: offline)
            return !offline
        } catch {
            Self.logger.info("ODR CDN not reachable. Listing which clips are actually on device.")
            publishInventory(playlist, unresolvedAsMissing: true)
            finishResolved(tag: tag, offline: true)
            return false
        }
    }

    private func finishResolved(tag: String, offline: Bool) {
        progressObserver?.cancel()
        progressObserver = nil
        activeTag = tag
        isDownloading = false
        usedOfflineFallback = offline
        stage = .ready
        lastError = offline ? "Some activity videos are not on this device yet." : nil
        let onDevice = clips.filter { $0.phase == .onDevice }.count
        let total = max(clips.count, 1)
        downloadProgress = Double(onDevice) / Double(total)
    }

    private func publishInventory(_ items: [StimulusPlaylistItem], unresolvedAsMissing: Bool = false) {
        clips = items.map { item in
            let phase: StimulusClipTransfer.Phase
            if resolveVideoURL(name: item.videoName) != nil {
                phase = .onDevice
            } else if unresolvedAsMissing {
                phase = .missing
            } else {
                phase = .waiting
            }
            return StimulusClipTransfer(id: item.videoName, title: item.title, phase: phase)
        }
        let onDevice = clips.filter { $0.phase == .onDevice }.count
        downloadProgress = clips.isEmpty ? 0 : Double(onDevice) / Double(clips.count)
    }

    private func setPhase(_ phase: StimulusClipTransfer.Phase, for id: String) {
        guard let index = clips.firstIndex(where: { $0.id == id }) else { return }
        clips[index].phase = phase
    }

    private func downloadClip(_ item: StimulusPlaylistItem, from base: URL) async {
        let fileName = item.videoName.hasSuffix(".mp4") ? item.videoName : "\(item.videoName).mp4"
        let remote = base.appendingPathComponent(fileName)
        let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        let destination = caches.appendingPathComponent(fileName)
        setPhase(.downloading(0), for: item.videoName)

        let fetcher = StimulusClipFetcher(destination: destination)
        fetcher.onProgress = { [weak self] fraction, written, expected in
            Task { @MainActor in
                self?.setPhase(.downloading(fraction), for: item.videoName)
                self?.downloadedBytes = written
                self?.totalBytes = expected
            }
        }
        do {
            _ = try await fetcher.fetch(remote)
            setPhase(.onDevice, for: item.videoName)
        } catch {
            Self.logger.error("Clip download failed for \(fileName, privacy: .public): \(error.localizedDescription, privacy: .public)")
            setPhase(.missing, for: item.videoName)
        }
    }

    // MARK: - Asset Resolution

    /// Resolves the local on-disk URL for a video file name.
    /// Checks:
    /// 1. App Bundle root or subdirectories (`Videos/`)
    /// 2. ODR Bundle (via `currentRequest?.bundle`)
    /// 3. Documents / Caches directory
    public func resolveVideoURL(name: String) -> URL? {
        let cleanName = name.replacingOccurrences(of: ".mp4", with: "")

        // 1. Check main bundle
        if let url = Bundle.main.url(forResource: cleanName, withExtension: "mp4") {
            return url
        }
        if let url = Bundle.main.url(forResource: cleanName, withExtension: "mp4", subdirectory: "Videos") {
            return url
        }

        // Direct bundle resource path check
        if let bundlePath = Bundle.main.resourcePath {
            let candidateInVideos = URL(fileURLWithPath: bundlePath).appendingPathComponent("Videos/\(cleanName).mp4")
            if FileManager.default.fileExists(atPath: candidateInVideos.path) {
                return candidateInVideos
            }
            let candidateRoot = URL(fileURLWithPath: bundlePath).appendingPathComponent("\(cleanName).mp4")
            if FileManager.default.fileExists(atPath: candidateRoot.path) {
                return candidateRoot
            }
        }

        // 2. Check ODR bundle if loaded
        if let odrBundle = currentRequest?.bundle,
           let url = odrBundle.url(forResource: cleanName, withExtension: "mp4") {
            return url
        }

        // 3. Check App Support / Caches directory (for external downloads)
        let fileManager = FileManager.default
        if let cachesDir = fileManager.urls(for: .cachesDirectory, in: .userDomainMask).first {
            let candidate = cachesDir.appendingPathComponent("\(cleanName).mp4")
            if fileManager.fileExists(atPath: candidate.path) {
                return candidate
            }
        }

        Self.logger.warning("Could not resolve local video file for: \(cleanName, privacy: .public)")
        return nil
    }

    /// Releases currently locked ODR assets to allow iOS to reclaim storage if needed.
    public func releaseCurrentPack() {
        progressObserver?.cancel()
        progressObserver = nil
        currentRequest?.endAccessingResources()
        currentRequest = nil
        activeTag = nil
        isDownloading = false
        downloadProgress = 0.0
        downloadedBytes = 0
        totalBytes = 0
        usedOfflineFallback = false
        stage = .idle
    }
}

private final class StimulusClipFetcher: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    var onProgress: (@Sendable (Double, Int64, Int64) -> Void)?
    private var continuation: CheckedContinuation<URL, Error>?
    private let destination: URL
    private lazy var session: URLSession = {
        URLSession(configuration: .ephemeral, delegate: self, delegateQueue: nil)
    }()

    init(destination: URL) {
        self.destination = destination
    }

    func fetch(_ url: URL) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            self.continuation = continuation
            session.downloadTask(with: url).resume()
        }
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64,
        totalBytesExpectedToWrite: Int64
    ) {
        let fraction = totalBytesExpectedToWrite > 0
            ? Double(totalBytesWritten) / Double(totalBytesExpectedToWrite)
            : 0
        onProgress?(fraction, totalBytesWritten, totalBytesExpectedToWrite)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        guard let continuation else { return }
        if let http = downloadTask.response as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
            self.continuation = nil
            continuation.resume(throwing: URLError(.badServerResponse))
            return
        }
        do {
            let folder = destination.deletingLastPathComponent()
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try? FileManager.default.removeItem(at: destination)
            try FileManager.default.moveItem(at: location, to: destination)
            self.continuation = nil
            continuation.resume(returning: destination)
        } catch {
            self.continuation = nil
            continuation.resume(throwing: error)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let error, let continuation else { return }
        self.continuation = nil
        continuation.resume(throwing: error)
    }
}

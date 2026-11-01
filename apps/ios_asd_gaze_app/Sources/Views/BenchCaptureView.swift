#if ASD_BENCH_CAPTURE
import ARKit
import AsdCore
import CurvAsdGazeKit
import SwiftUI

/// Bench capture screen (WP-G.8, BEN-2). Debug builds only (`ASD_BENCH_CAPTURE`).
///
/// Runs a 5-point or 9-point calibration sequence or the 13-point grid with fixed targets and
/// fixed timing (no gaze feedback), records every ARKit frame through the `ARSessionBridge` tap
/// and writes `asd-raw-tracking/1` JSONL plus a target log to Documents/BenchLogs for manual
/// export (share sheet). No images and no video are recorded (BEN-2); the logs are biometric
/// data and never go to git (BEN-8).
struct BenchCaptureView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var model = BenchCaptureModel()

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            switch model.phase {
            case .idle, .failed, .finished:
                setupForm
            case .running:
                runningScreen
            }
        }
        .preferredColorScheme(.dark)
        .onDisappear { model.cancel() }
    }

    // MARK: - Setup

    private var setupForm: some View {
        Form {
            Section("Sequence") {
                Picker("Sequence", selection: $model.sequence) {
                    ForEach(BenchSequence.allCases) { Text($0.title).tag($0) }
                }
                .pickerStyle(.inline)
                .labelsHidden()
            }
            Section("Condition (BEN-4)") {
                // A closed list from Rust: no free text (and no personal data) reaches a file
                // name or the log header.
                Picker("Condition", selection: $model.condition) {
                    ForEach(BenchCaptureModel.conditions, id: \.self) { condition in
                        Text(gazeBenchConditionName(condition: condition)).tag(condition)
                    }
                }
                .accessibilityIdentifier("bench_condition")
            }
            Section {
                Button("Start capture") { model.start() }
                    .accessibilityIdentifier("bench_start")
                Button("Close") { dismiss() }
            } footer: {
                Text("Device on a stand at the measured distance. Targets run in fixed order and timing; triple-tap the screen to cancel. Files stay on this device until you share them.")
            }
            switch model.phase {
            case let .failed(message):
                Section("Problem") { Text(message).foregroundStyle(.red) }
            case let .finished(files):
                Section("Saved") {
                    Text("\(files.sampleCount) samples, \(files.droppedSampleCount) dropped (repeated timestamp)")
                    Text(files.rawLogURL.lastPathComponent).font(.footnote.monospaced())
                    ShareLink("Share log", items: [files.rawLogURL, files.targetLogURL])
                }
            default:
                EmptyView()
            }
        }
        .scrollContentBackground(.hidden)
    }

    // MARK: - Running

    private var runningScreen: some View {
        GeometryReader { proxy in
            ZStack {
                Color.black
                if let target = model.currentTarget {
                    BenchTargetDot()
                        .position(x: target.xNorm * proxy.size.width, y: target.yNorm * proxy.size.height)
                } else {
                    Text("Look at each dot as it appears")
                        .foregroundStyle(.white.opacity(0.7))
                }
            }
            .contentShape(Rectangle())
            .onTapGesture(count: 3) { model.cancel() }
        }
        .ignoresSafeArea()
        .statusBarHidden()
    }
}

private struct BenchTargetDot: View {
    var body: some View {
        ZStack {
            Circle().fill(Color.white).frame(width: 28, height: 28)
            Circle().fill(Color.red).frame(width: 6, height: 6)
        }
        .accessibilityIdentifier("bench_target")
    }
}

// MARK: - Model

@MainActor
@Observable
final class BenchCaptureModel {
    enum Phase: Equatable {
        case idle
        case running
        case finished(BenchLogFiles)
        case failed(String)
    }

    var sequence: BenchSequence = .grid13
    /// BEN-4 conditions in spec order (Rust `gazeBenchConditions()`).
    static let conditions = gazeBenchConditions()

    var condition: GazeBenchCondition = .baseline
    private(set) var phase: Phase = .idle
    private(set) var currentTarget: BenchPlannedTarget?

    private var session: ARSession?
    private var bridge: ARSessionBridge?
    private var runTask: Task<Void, Never>?
    private let arQueue = DispatchQueue(label: "org.curv.asd.bench.arQueue", qos: .userInteractive)

    /// Seconds of face-tracking warm-up before the first target.
    private static let leadInSec = 2.0

    func start() {
        guard phase != .running else { return }
        guard ARFaceTrackingConfiguration.isSupported else {
            phase = .failed("Face tracking is not supported on this device (the simulator has no TrueDepth camera).")
            return
        }
        let orientation = GazeDeviceInfo.currentInterfaceOrientation()
        let config: BenchRunConfig
        do {
            config = try BenchRunConfig.load(modelIdentifier: GazeDeviceInfo.modelIdentifier, orientation: orientation)
        } catch {
            phase = .failed("Could not read the gaze config: \(error)")
            return
        }
        let stamp = Self.fileStamp()
        let conditionName = gazeBenchConditionName(condition: condition)
        let writer = BenchLogWriter(
            directory: Self.logDirectory,
            baseName: "bench-\(conditionName)-\(sequence.rawValue)-\(stamp)",
            header: config.header(
                orientation: orientation,
                appVersion: SettingsView.appVersionLabel,
                condition: condition
            )
        )

        let bridge = ARSessionBridge(processingActor: GazeProcessingActor())
        bridge.updateOrientation(orientation)
        bridge.setBenchTap { writer.append($0) }
        let session = ARSession()
        session.delegate = bridge
        session.delegateQueue = arQueue
        self.bridge = bridge
        self.session = session
        phase = .running
        currentTarget = nil
        UIApplication.shared.isIdleTimerDisabled = true
        session.run(ARFaceTrackingConfiguration(), options: [.resetTracking, .removeExistingAnchors])

        let plan = sequence.plan(config)
        let dwell = config.settleSec + config.collectSec
        runTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(Self.leadInSec))
            for target in plan {
                guard !Task.isCancelled, let self else { return }
                bridge.updateOrientation(GazeDeviceInfo.currentInterfaceOrientation())
                self.currentTarget = target
                // The target clock is the ARKit frame clock (system uptime) so the schedule
                // and the samples share t_sec.
                let start = BenchRawSample.frameClockNow
                try? await Task.sleep(for: .seconds(dwell))
                let end = BenchRawSample.frameClockNow
                writer.recordTarget(BenchTargetRecord(
                    kind: target.kind, xNorm: target.xNorm, yNorm: target.yNorm,
                    startSec: start, endSec: end, label: target.label
                ))
            }
            guard !Task.isCancelled, let self else { return }
            self.stopSession()
            do {
                self.phase = .finished(try writer.finish())
            } catch {
                self.phase = .failed("Could not write the log: \(error)")
            }
        }
    }

    /// Stops a running capture and discards it (nothing is written).
    func cancel() {
        guard phase == .running else { return }
        runTask?.cancel()
        stopSession()
        phase = .idle
    }

    private func stopSession() {
        runTask = nil
        bridge?.setBenchTap(nil)
        session?.pause()
        session?.delegate = nil
        session = nil
        bridge = nil
        currentTarget = nil
        UIApplication.shared.isIdleTimerDisabled = false
    }

    /// Shared with `DataDeletionService`, which removes it on "Delete all data".
    static var logDirectory: URL {
        DataDeletionService.defaultBenchLogDirectory()
    }

    private static func fileStamp() -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyyMMdd-HHmmss"
        return f.string(from: Date())
    }
}
#endif

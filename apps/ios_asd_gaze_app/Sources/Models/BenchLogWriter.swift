#if ASD_BENCH_CAPTURE
import AsdCore
import CurvAsdGazeKit
import Foundation
import os

/// Bench capture build (WP-G.8, BEN-2, ADR-0009 D10). Compiled only under `ASD_BENCH_CAPTURE`,
/// which is defined in Debug and nowhere else; `scripts/check_release_symbols.sh` fails if any
/// of this reaches a Release or DemoRelease binary.
///
/// Everything here is glue. The format (`asd-raw-tracking/1`: header, sample lines, target
/// log), the header geometry, the BEN-4 condition names and the target layouts (5, 9, 13
/// points) are defined in Rust (`asd-core`, encoded by asd-ffi `raw_log`) and reached through
/// UniFFI; Swift builds no JSON of the log and computes no gaze or grid maths.
enum BenchCaptureFlag {
    /// The literal is the needle `scripts/check_release_symbols.sh` greps for.
    static let name = "ASD_BENCH_CAPTURE"
}

/// One entry of the header's target schedule and of the target log (REP-1 `ScheduledTarget`).
struct BenchTargetRecord: Equatable, Sendable {
    var kind: GazeTargetKind
    var xNorm: Double
    var yNorm: Double
    var startSec: Double
    var endSec: Double
    var label: String?

    var scheduled: GazeScheduledTarget {
        GazeScheduledTarget(kind: kind, xNorm: xNorm, yNorm: yNorm, startSec: startSec, endSec: endSec, label: label)
    }
}

/// Header fields known before recording starts. Rust resolves the geometry for the model
/// (table entry or idiom default, GEO-2) and writes the `RawLogHeader`.
struct BenchLogHeader: Sendable {
    /// Hardware model identifier (`utsname.machine`).
    var modelIdentifier: String
    var orientation: GazeOrientation
    var appVersion: String?
    /// A BEN-4 condition from `gazeBenchConditions()`, never free text.
    var condition: GazeBenchCondition?

    func rustInput(targets: [BenchTargetRecord]) -> GazeRawLogHeaderInput {
        GazeRawLogHeaderInput(
            modelIdentifier: modelIdentifier,
            orientation: orientation,
            targets: targets.map(\.scheduled),
            appVersion: appVersion,
            condition: condition
        )
    }
}

struct BenchLogFiles: Sendable, Equatable {
    var rawLogURL: URL
    var targetLogURL: URL
    var sampleCount: Int
    var droppedSampleCount: Int
}

/// Writes one bench recording: `<base>.jsonl` (REP-1) and `<base>.targets.json`.
///
/// `append` is called on the AR delegate queue; `recordTarget` and `finish` on the main actor.
/// Frames are kept in memory (a bench run is minutes, a few MB) and encoded once at `finish`
/// by Rust (`gazeRawLogDocument`), because the header carries the target schedule, which is
/// only known at the end. Rust keeps `t_sec` strictly increasing and counts what it drops.
final class BenchLogWriter: Sendable {
    private struct State: Sendable {
        var records: [RawSampleRecord] = []
        var targets: [BenchTargetRecord] = []
    }

    private let directory: URL
    private let baseName: String
    private let header: BenchLogHeader
    private let state = OSAllocatedUnfairLock(initialState: State())

    init(directory: URL, baseName: String, header: BenchLogHeader) {
        self.directory = directory
        self.baseName = Self.sanitisedName(baseName)
        self.header = header
    }

    // MARK: - Recording

    func append(_ sample: BenchRawSample) {
        let record = RawSampleRecord(
            snapshot: sample.snapshot,
            tSec: sample.timestamp,
            orientation: sample.orientation,
            clipMediaTimeSec: nil,
            stimulusId: nil
        )
        state.withLock { $0.records.append(record) }
    }

    func recordTarget(_ target: BenchTargetRecord) {
        state.withLock { $0.targets.append(target) }
    }

    /// Writes both files (complete file protection, directory excluded from backup: BEN-8).
    func finish() throws -> BenchLogFiles {
        let snapshot = state.withLock { $0 }
        let document = try gazeRawLogDocument(
            headerInput: header.rustInput(targets: snapshot.targets),
            records: snapshot.records
        )
        let targetLog = try gazeScheduledTargetsJson(targets: snapshot.targets.map(\.scheduled))

        let fm = FileManager.default
        try fm.createDirectory(at: directory, withIntermediateDirectories: true)
        var dir = directory
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try dir.setResourceValues(values)

        let rawURL = directory.appendingPathComponent("\(baseName).jsonl")
        let targetURL = directory.appendingPathComponent("\(baseName).targets.json")
        try Data(document.jsonl.utf8).write(to: rawURL, options: [.atomic, .completeFileProtection])
        try Data(targetLog.utf8).write(to: targetURL, options: [.atomic, .completeFileProtection])
        return BenchLogFiles(
            rawLogURL: rawURL,
            targetLogURL: targetURL,
            sampleCount: Int(document.sampleCount),
            droppedSampleCount: Int(document.droppedSampleCount)
        )
    }

    /// File-name safe text: ASCII letters, digits, `-` and `_`; everything else becomes `-`.
    /// The parts of a bench base name are already safe (Rust condition name, sequence, time
    /// stamp); this only guards against a future caller passing something else.
    static func sanitisedName(_ text: String) -> String {
        String(text.unicodeScalars.map { scalar in
            let ok = (scalar.value < 128) && (CharacterSet.alphanumerics.contains(scalar) || scalar == "-" || scalar == "_")
            return ok ? Character(scalar) : "-"
        })
    }
}

// MARK: - Run configuration from Rust

/// Layouts and timings the bench screen needs, read from `gazeDefaultConfigJson`
/// (`PipelineConfig`): `params.calibration.layout5|layout9`, `params.validation.benchLayout13`
/// (VAL-5) and the CAL-8 window. The header geometry is resolved by Rust when the log is
/// written.
struct BenchRunConfig: Sendable {
    var modelIdentifier: String
    var geometrySource: String
    var layout5: [[Double]]
    var layout9: [[Double]]
    var grid13: [[Double]]
    var settleSec: Double
    var collectSec: Double

    enum LoadError: Error { case malformedConfig(String) }

    static func load(modelIdentifier: String, orientation: GazeOrientation) throws -> BenchRunConfig {
        let text = try gazeDefaultConfigJson(modelIdentifier: modelIdentifier, orientation: orientation)
        guard let root = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              let source = root["geometrySource"] as? String,
              let params = root["params"] as? [String: Any],
              let calibration = params["calibration"] as? [String: Any],
              let validation = params["validation"] as? [String: Any],
              let layout5 = calibration["layout5"] as? [[Double]],
              let layout9 = calibration["layout9"] as? [[Double]],
              let grid13 = validation["benchLayout13"] as? [[Double]],
              let settleMs = calibration["settleMs"] as? Double,
              let collectMs = calibration["collectMs"] as? Double
        else { throw LoadError.malformedConfig("missing layouts or window") }
        return BenchRunConfig(
            modelIdentifier: (root["modelIdentifier"] as? String) ?? modelIdentifier,
            geometrySource: source,
            layout5: layout5, layout9: layout9, grid13: grid13,
            settleSec: settleMs / 1000, collectSec: collectMs / 1000
        )
    }

    func header(orientation: GazeOrientation, appVersion: String?, condition: GazeBenchCondition) -> BenchLogHeader {
        BenchLogHeader(
            modelIdentifier: modelIdentifier, orientation: orientation,
            appVersion: appVersion, condition: condition
        )
    }
}

/// A target shown on the bench screen: fixed order, fixed timing, no gaze feedback.
struct BenchPlannedTarget: Equatable, Sendable {
    var kind: GazeTargetKind
    var xNorm: Double
    var yNorm: Double
    var label: String
}

/// The three bench sequences (BEN-2). 5 and 9 points are calibration targets (CAL-7); the
/// 13-point grid (VAL-5) is validation targets.
enum BenchSequence: String, CaseIterable, Identifiable, Sendable {
    case fivePoint
    case ninePoint
    case grid13

    var id: String { rawValue }

    var title: String {
        switch self {
        case .fivePoint: "5-point calibration"
        case .ninePoint: "9-point calibration"
        case .grid13: "13-point grid"
        }
    }

    func plan(_ config: BenchRunConfig) -> [BenchPlannedTarget] {
        let (points, kind, prefix): ([[Double]], GazeTargetKind, String) = switch self {
        case .fivePoint: (config.layout5, .calibration, "cal5")
        case .ninePoint: (config.layout9, .calibration, "cal9")
        case .grid13: (config.grid13, .validation, "grid13")
        }
        return points.enumerated().map { index, p in
            BenchPlannedTarget(kind: kind, xNorm: p[0], yNorm: p[1], label: "\(prefix)-\(index + 1)")
        }
    }
}
#endif

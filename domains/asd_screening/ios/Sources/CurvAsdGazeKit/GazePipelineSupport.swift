import AsdCore
import Foundation
import simd
#if canImport(UIKit) && !os(watchOS)
import UIKit
#endif

// Swift glue around the Rust per-session gaze pipeline (WP-G.9, PIPE-1, ADR-0007 D2).
//
// Nothing here computes gaze: it converts ARKit values into the flat FFI records, carries the
// Rust documents (`trackingQuality`, per-clip `ClipTracking`) through Swift unchanged, and
// names the device. Raycast, calibration apply, track state, confidence, 1-Euro filtering,
// AOI hit testing, gap filling and the session summary all run in `asd-core::gaze`.

// MARK: - Embedded JSON

/// A JSON value carried verbatim (the Rust `trackingQuality` record, QS-1). Swift reads none of
/// its fields for logic; it only stores and re-encodes the document.
public enum JSONValue: Codable, Sendable, Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let b = try? c.decode(Bool.self) {
            self = .bool(b)
        } else if let n = try? c.decode(Double.self) {
            self = .number(n)
        } else if let s = try? c.decode(String.self) {
            self = .string(s)
        } else if let a = try? c.decode([JSONValue].self) {
            self = .array(a)
        } else {
            self = .object(try c.decode([String: JSONValue].self))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case let .bool(b): try c.encode(b)
        case let .number(n): try c.encode(n)
        case let .string(s): try c.encode(s)
        case let .array(a): try c.encode(a)
        case let .object(o): try c.encode(o)
        }
    }

    /// Parses a JSON document.
    public init(json: String) throws {
        self = try JSONDecoder().decode(JSONValue.self, from: Data(json.utf8))
    }

    /// The document as JSON text (sorted keys).
    public func jsonString() throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return String(decoding: try encoder.encode(self), as: UTF8.self)
    }

    public subscript(key: String) -> JSONValue? {
        if case let .object(o) = self { return o[key] }
        return nil
    }
}

// MARK: - Per-clip tracking (QS-3, CNF-3)

/// The four 1.7 per-clip tracking values (`ClipTracking` in `asd-contract`), each absent when
/// not measured (CON-14). Produced by the Rust pipeline; Swift only carries them.
public struct ClipTracking: Codable, Sendable, Equatable {
    public let meanSampleConfidence: Double?
    public let meanFixationConfidence: Double?
    public let nearestCheckOffsetDeg: Double?
    public let aoiMinSizeDeg: Double?

    public init(
        meanSampleConfidence: Double? = nil,
        meanFixationConfidence: Double? = nil,
        nearestCheckOffsetDeg: Double? = nil,
        aoiMinSizeDeg: Double? = nil
    ) {
        self.meanSampleConfidence = meanSampleConfidence
        self.meanFixationConfidence = meanFixationConfidence
        self.nearestCheckOffsetDeg = nearestCheckOffsetDeg
        self.aoiMinSizeDeg = aoiMinSizeDeg
    }
}

/// One entry of `GazePipelineHandle.clipTrackingJson()`: `{clipId, order, ...ClipTracking}`.
public struct ClipTrackingEntry: Codable, Sendable, Equatable {
    public let clipId: String
    /// 1-based play order.
    public let order: Int
    public let tracking: ClipTracking

    public init(clipId: String, order: Int, tracking: ClipTracking) {
        self.clipId = clipId
        self.order = order
        self.tracking = tracking
    }

    private enum CodingKeys: String, CodingKey { case clipId, order }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        clipId = try c.decode(String.self, forKey: .clipId)
        order = try c.decode(Int.self, forKey: .order)
        tracking = try ClipTracking(from: decoder)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(clipId, forKey: .clipId)
        try c.encode(order, forKey: .order)
        try tracking.encode(to: encoder)
    }
}

extension PerVideoScoreReport {
    /// The report with the clip's QS-3 tracking values.
    public func withTracking(_ t: ClipTracking) -> PerVideoScoreReport {
        PerVideoScoreReport(
            stimulusId: stimulusId,
            title: title,
            category: category,
            rows: rows,
            subscore: subscore,
            validSampleRatio: validSampleRatio,
            lowConfidence: lowConfidence,
            hasBaseline: hasBaseline,
            weight: weight,
            meanSampleConfidence: t.meanSampleConfidence,
            meanFixationConfidence: t.meanFixationConfidence,
            nearestCheckOffsetDeg: t.nearestCheckOffsetDeg,
            aoiMinSizeDeg: t.aoiMinSizeDeg
        )
    }

    /// Attaches the pipeline's per-clip values to the score reports (1.7 `perVideo[]`).
    ///
    /// A report takes the entry whose `clipId` is its `stimulusId`. When a clip played more
    /// than once (several entries with that id) the entry whose play `order` equals the
    /// report's position is used; when none does, the report stays without tracking values
    /// rather than borrowing another play's (absent = not measured, CON-14).
    public static func merging(
        _ reports: [PerVideoScoreReport],
        clipTracking: [ClipTrackingEntry]
    ) -> [PerVideoScoreReport] {
        reports.enumerated().map { index, report in
            let matches = clipTracking.filter { $0.clipId == report.stimulusId }
            let entry = matches.count == 1 ? matches.first : matches.first { $0.order == index + 1 }
            return entry.map { report.withTracking($0.tracking) } ?? report
        }
    }
}

// MARK: - Session summary documents

extension GazeSessionReportSummary {
    /// QS-5: the calibration pose distance when a calibration ran in this session, `nil`
    /// otherwise (computed in Rust, `gaze_calibrated_distance_cm`). Never a constant.
    public var calibratedDistanceCm: Double? {
        guard let trackingQuality, let json = try? trackingQuality.jsonString() else { return nil }
        do {
            return try gazeCalibratedDistanceCm(trackingQualityJson: json)
        } catch {
            AsdCoreBridge.report("gaze_calibrated_distance_cm", error)
            return nil
        }
    }
}

// MARK: - Per-frame track state (TRK-1)

/// Per-frame track state written into the frames file (`track_state`, SCH-1).
public enum FrameTrackState: String, Codable, Sendable {
    case tracked
    case blink
    case lost
    case offscreen
    case artifact

    init(_ state: GazeTrackState) {
        switch state {
        case .tracked: self = .tracked
        case .blink: self = .blink
        case .lost: self = .lost
        case .offscreen: self = .offscreen
        case .artifact: self = .artifact
        }
    }
}

// MARK: - Device and orientation (GEO-2, GEO-3)

public enum GazeDeviceInfo {
    /// Hardware model identifier (`utsname.machine`, e.g. `iPhone17,3`) for the geometry table
    /// (GEO-2). On the simulator, the simulated device's identifier.
    public static let modelIdentifier: String = {
        if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"], !simulated.isEmpty {
            return simulated
        }
        var info = utsname()
        uname(&info)
        return withUnsafeBytes(of: &info.machine) { raw in
            String(decoding: raw.prefix { $0 != 0 }, as: UTF8.self)
        }
    }()

    #if canImport(UIKit) && !os(watchOS) && !os(visionOS)
    /// The interface orientation of the foreground scene (main actor only); portrait when
    /// unknown.
    @MainActor
    public static func currentInterfaceOrientation() -> GazeOrientation {
        let scene = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }
        return GazeOrientation(scene?.interfaceOrientation ?? .portrait)
    }
    #endif
}

#if canImport(UIKit) && !os(watchOS)
extension GazeOrientation {
    /// `UIInterfaceOrientation` to the pipeline orientation; unknown reads as portrait.
    public init(_ orientation: UIInterfaceOrientation) {
        switch orientation {
        case .portraitUpsideDown: self = .portraitUpsideDown
        case .landscapeLeft: self = .landscapeLeft
        case .landscapeRight: self = .landscapeRight
        default: self = .portrait
        }
    }
}
#endif

// MARK: - FFI records

extension simd_float4x4 {
    /// Column-major, 16 numbers (`simd_float4x4` columns in order), as `RawSampleRecord` wants.
    var columnMajor: [Double] {
        [
            Double(columns.0.x), Double(columns.0.y), Double(columns.0.z), Double(columns.0.w),
            Double(columns.1.x), Double(columns.1.y), Double(columns.1.z), Double(columns.1.w),
            Double(columns.2.x), Double(columns.2.y), Double(columns.2.z), Double(columns.2.w),
            Double(columns.3.x), Double(columns.3.y), Double(columns.3.z), Double(columns.3.w)
        ]
    }
}

extension GazeVec3 {
    init(_ v: simd_float3) {
        self.init(x: Double(v.x), y: Double(v.y), z: Double(v.z))
    }
}

extension RawSampleRecord {
    /// One ARKit frame for `GazePipelineHandle.push` (PIPE-2). A frame without a tracked face
    /// carries no pose (the pipeline ignores it anyway, TRK-1). Public for the app's bench
    /// capture, which hands the same records to the Rust raw-log encoder (REP-1).
    public init(
        snapshot: FaceAnchorSnapshot,
        tSec: Double,
        orientation: GazeOrientation,
        clipMediaTimeSec: Double?,
        stimulusId: String?
    ) {
        let tracked = snapshot.isTracked
        self.init(
            tSec: tSec,
            faceTracked: tracked,
            faceTransform: tracked ? snapshot.transform.columnMajor : nil,
            lookAt: tracked ? GazeVec3(snapshot.lookAtPoint) : nil,
            leftEyeTransform: tracked ? snapshot.leftEyeTransform.columnMajor : nil,
            rightEyeTransform: tracked ? snapshot.rightEyeTransform.columnMajor : nil,
            eyeBlinkLeft: snapshot.blendShapes["eyeBlinkLeft"],
            eyeBlinkRight: snapshot.blendShapes["eyeBlinkRight"],
            ambientIntensity: snapshot.ambientIntensity,
            ambientColorTemp: snapshot.ambientColorTemperature,
            lightDirection: snapshot.lightDirection.map(GazeVec3.init),
            orientation: orientation,
            clipMediaTimeSec: clipMediaTimeSec,
            stimulusId: stimulusId
        )
    }
}

// MARK: - Documents for the pipeline

enum GazePipelineDocuments {
    /// `PipelineConfig` JSON for this device with `qualityGate` and `params` from the delivered
    /// `ScoringProfile` (CFG-3, WP-A.13). Rust builds the whole document: the table geometry
    /// or idiom default, and the profile's gate and `trackingQuality` (the asd-core defaults
    /// for a profile that predates 1.1.0). Swift only passes the profile JSON through; Rust
    /// throws for a profile that fails the publisher's value checks (WP-A.13).
    static func config(
        modelIdentifier: String,
        orientation: GazeOrientation,
        scoringProfileJSON: String
    ) throws -> String {
        try gazeConfigJsonForProfile(
            modelIdentifier: modelIdentifier,
            orientation: orientation,
            scoringProfileJson: scoringProfileJSON
        )
    }

    /// The `qualityGate` object of a `PipelineConfig` JSON, verbatim, for the frames-file
    /// writer (`GazeTelemetryWriterHandle`), so the writer gates with exactly the parameters
    /// the pipeline was built with. Swift reads none of its fields.
    static func qualityGateJSON(ofConfig configJSON: String) throws -> String? {
        try JSONValue(json: configJSON)["qualityGate"]?.jsonString()
    }

    /// The kit's affine profile as a Rust `CalModel` (`screenAffine`, the same `a..f`), or `nil`
    /// for an uncalibrated profile. The app's views calibrate through the pipeline (WP-G.10:
    /// Rust fits and records the model), so they no longer pass a fitted profile here; this path
    /// remains only for callers that inject a precomputed profile, which the pipeline records
    /// as `skipped`.
    static func calibrationModel(_ profile: GazeCalibrationProfile) throws -> String? {
        guard profile.isCalibrated else { return nil }
        struct ScreenAffine: Encodable {
            let kind = "screenAffine"
            let x: [Double]
            let y: [Double]
        }
        let model = ScreenAffine(x: [profile.a, profile.b, profile.c], y: [profile.d, profile.e, profile.f])
        return String(decoding: try JSONEncoder().encode(model), as: UTF8.self)
    }

    /// Catalog areas (`StimulusCatalog.Area` JSON, with roles) of a clip as screen-space AOIs
    /// (`[ScreenAoi]` JSON). Areas come from the catalog when it knows the clip, otherwise
    /// from the battery's boxes (no roles). Catalog boxes are screen space today (trap 11).
    static func clipAOIs(battery: StimulusAOITracker, stimulusId: String?) throws -> String {
        let areas: [StimulusCatalog.Area]
        if let id = stimulusId,
           let clip = StimulusCatalog.current.clip(id: id.replacingOccurrences(of: ".mp4", with: "")),
           !clip.areas.isEmpty {
            areas = clip.areas
        } else {
            areas = battery.aois.map {
                StimulusCatalog.Area(
                    name: $0.name,
                    startSec: $0.startTimeSec,
                    endSec: $0.endTimeSec,
                    xMin: $0.xMin,
                    yMin: $0.yMin,
                    xMax: $0.xMax,
                    yMax: $0.yMax,
                    roles: []
                )
            }
        }
        let json = String(decoding: try JSONEncoder().encode(areas), as: UTF8.self)
        return try gazeClipAoisJson(
            areasJson: json,
            aoiSpace: .screen,
            videoAspect: 1,
            screenAspect: 1,
            gravity: .aspectFill
        )
    }
}

import AsdCore
import Foundation
import simd
import XCTest
@testable import CurvAsdGazeKit

/// PIPE-4: per-sample cost of `push` through the FFI, reported as p50 / p99 (milliseconds).
/// The budget is 0.2 ms at p99 on the slowest supported device (DEV-2); this host run (macOS,
/// `swift test`) and the simulator run in the app tests (`GazePipelineCostTests` there) are
/// indicative only, the device number comes from the same test on hardware (acceptance 12).
///
/// Three costs per frame: `push` alone (the PIPE-4 figure), `push` + frames-file `write`, and
/// the whole actor path (`processFaceAnchor`: record building, FFI, writer, affect, UI state,
/// including the actor hop).
final class GazePipelineCostTests: XCTestCase {
    static let budgetMs = 0.2

    /// A head swaying slowly with saccades, blinks every 2 s and a 100 ms loss every 5 s:
    /// every branch of `push` runs (tracked, blink, lost, filter, AOI hit test).
    static func frame(_ i: Int) -> FaceAnchorSnapshot {
        let t = Double(i) / 60.0
        if i % 300 >= 294 {
            return FaceAnchorSnapshot(isTracked: false, transform: matrix_identity_float4x4, lookAtPoint: simd_float3(0, 0, 0))
        }
        let yaw = Float(0.05 * sin(t * 0.7))
        var transform = simd_float4x4(simd_quatf(angle: yaw, axis: simd_float3(0, 1, 0)))
        transform.columns.3 = simd_float4(Float(0.01 * sin(t * 0.3)), -0.02, -0.45 + Float(0.02 * sin(t * 0.2)), 1)
        let saccade = Float((i / 40) % 3) - 1
        let look = simd_float3(0.03 * saccade, -0.1 + 0.01 * Float(sin(t * 5)), 0.45)
        var eye = matrix_identity_float4x4
        eye.columns.3 = simd_float4(0.031, 0.025, 0.02, 1)
        var right = eye
        right.columns.3.x = -0.031
        let blink = i % 120 < 6 ? 0.9 : 0.05
        return FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: look,
            leftEyeTransform: eye,
            rightEyeTransform: right,
            blendShapes: ["eyeBlinkLeft": blink, "eyeBlinkRight": blink, "mouthSmileLeft": 0.2],
            ambientIntensity: 900,
            ambientColorTemperature: 5200
        )
    }

    static func percentile(_ sorted: [Double], _ p: Double) -> Double {
        sorted[min(sorted.count - 1, Int((Double(sorted.count - 1) * p).rounded()))]
    }

    static func report(_ name: String, _ samplesMs: [Double]) -> (p50: Double, p99: Double) {
        let sorted = samplesMs.sorted()
        let p50 = percentile(sorted, 0.50)
        let p99 = percentile(sorted, 0.99)
        let line = String(format: "PIPE-4 %@: n=%d p50=%.4f ms p99=%.4f ms max=%.4f ms (budget p99 <= %.1f ms on device)",
                          name, sorted.count, p50, p99, sorted.last ?? 0, budgetMs)
        print(line)
        return (p50, p99)
    }

    /// `swift test --sanitize=thread` links the TSan runtime.
    static let underThreadSanitizer: Bool = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "__tsan_init") != nil

    static func nowMs() -> Double {
        Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000
    }

    func testPerFrameCostThroughTheFFI() throws {
        let frames = 3_600 // one minute at 60 Hz
        let config = try GazePipelineDocuments.config(modelIdentifier: "iPhone17,3", orientation: .portrait, scoringProfileJSON: AsdCoreBridge.defaultScoringProfileJSON)
        let pipeline = try GazePipelineHandle(configJson: config)
        let writer = try GazeTelemetryWriterHandle(originSec: 0, qualityGateJson: nil)
        try pipeline.beginClipJson(clipId: "bench", aoisJson: GazePipelineDocuments.clipAOIs(
            battery: StimulusAOITracker.makeFaceScanningBattery(), stimulusId: nil))
        pipeline.setRecording(on: true)
        let raws = (0..<frames).map {
            RawSampleRecord(snapshot: Self.frame($0), tSec: Double($0) / 60, orientation: .portrait,
                            clipMediaTimeSec: Double($0) / 60, stimulusId: "bench")
        }
        // Warm-up (first-call allocations), then measure on a fresh pipeline.
        for raw in raws.prefix(120) { _ = writer.write(sample: try pipeline.push(raw: raw)) }

        let measured = try GazePipelineHandle(configJson: config)
        try measured.beginClipJson(clipId: "bench", aoisJson: "[]")
        measured.setRecording(on: true)
        var pushMs: [Double] = []
        var pushWriteMs: [Double] = []
        pushMs.reserveCapacity(frames)
        pushWriteMs.reserveCapacity(frames)
        for raw in raws {
            let t0 = Self.nowMs()
            let p = try measured.push(raw: raw)
            let t1 = Self.nowMs()
            _ = writer.write(sample: p)
            let t2 = Self.nowMs()
            pushMs.append(t1 - t0)
            pushWriteMs.append(t2 - t0)
        }
        let push = Self.report("push (FFI)", pushMs)
        _ = Self.report("push + write (FFI)", pushWriteMs)
        // Regression guard only: the host is faster than any supported device, so a host p99
        // above the device budget means something is badly wrong (debug or release core).
        // ThreadSanitizer slows the Swift half of the call several times; there it only reports.
        if !Self.underThreadSanitizer {
            XCTAssertLessThan(push.p99, Self.budgetMs, "host p99 of push exceeds the device budget")
        }
    }

    func testPerFrameCostOfTheActorPath() async throws {
        let actor = GazeProcessingActor(deviceModelIdentifier: "iPhone17,3")
        await actor.startSession(battery: StimulusAOITracker.makeFaceScanningBattery(), stimulusId: "bench")
        for i in 0..<120 { _ = await actor.processFaceAnchor(snapshot: Self.frame(i), elapsedSec: Double(i) / 60) }
        var ms: [Double] = []
        for i in 120..<3_720 {
            let snapshot = Self.frame(i)
            let t0 = Self.nowMs()
            _ = await actor.processFaceAnchor(snapshot: snapshot, elapsedSec: Double(i) / 60)
            ms.append(Self.nowMs() - t0)
        }
        _ = Self.report("actor processFaceAnchor", ms)
        let (_, frames) = await actor.finalizeSession()
        XCTAssertEqual(frames.count, 3_720)
    }

    /// The XCTest metric form for device runs (acceptance 12): 600 frames per iteration.
    func testPushClockMetric() throws {
        let config = try GazePipelineDocuments.config(modelIdentifier: "iPhone17,3", orientation: .portrait, scoringProfileJSON: AsdCoreBridge.defaultScoringProfileJSON)
        let raws = (0..<600).map {
            RawSampleRecord(snapshot: Self.frame($0), tSec: Double($0) / 60, orientation: .portrait,
                            clipMediaTimeSec: nil, stimulusId: nil)
        }
        let options = XCTMeasureOptions()
        options.iterationCount = 5
        measure(metrics: [XCTClockMetric()], options: options) {
            guard let pipeline = try? GazePipelineHandle(configJson: config) else { return XCTFail("config") }
            pipeline.setRecording(on: true)
            for raw in raws { _ = try? pipeline.push(raw: raw) }
        }
    }
}

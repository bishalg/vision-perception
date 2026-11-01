import AsdCore
import CurvAsdGazeKit
import Foundation
import simd
import XCTest

/// PIPE-4 on the iOS simulator (WP-G.9): per-frame cost of the Rust gaze pipeline through the
/// FFI, as p50 / p99 in milliseconds, plus the whole actor path. The budget (p99 <= 0.2 ms) is
/// defined on the slowest supported device (DEV-2); run this test on that device for the
/// acceptance number (acceptance 12). On the simulator it is a regression guard: the host
/// CPU is faster than any supported phone.
final class GazePipelineCostTests: XCTestCase {
    static let budgetMs = 0.2

    /// Column-major 4x4 for the FFI record.
    static func columns(_ m: simd_float4x4) -> [Double] {
        [m.columns.0, m.columns.1, m.columns.2, m.columns.3].flatMap { [Double($0.x), Double($0.y), Double($0.z), Double($0.w)] }
    }

    /// A swaying head with saccades, a blink every 2 s and a 100 ms loss every 5 s.
    static func snapshot(_ i: Int) -> FaceAnchorSnapshot {
        let t = Double(i) / 60.0
        if i % 300 >= 294 {
            return FaceAnchorSnapshot(isTracked: false, transform: matrix_identity_float4x4, lookAtPoint: simd_float3(0, 0, 0))
        }
        var transform = simd_float4x4(simd_quatf(angle: Float(0.05 * sin(t * 0.7)), axis: simd_float3(0, 1, 0)))
        transform.columns.3 = simd_float4(Float(0.01 * sin(t * 0.3)), -0.02, -0.45 + Float(0.02 * sin(t * 0.2)), 1)
        var left = matrix_identity_float4x4
        left.columns.3 = simd_float4(0.031, 0.025, 0.02, 1)
        var right = left
        right.columns.3.x = -0.031
        let blink = i % 120 < 6 ? 0.9 : 0.05
        return FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.03 * (Float((i / 40) % 3) - 1), -0.1, 0.45),
            leftEyeTransform: left,
            rightEyeTransform: right,
            blendShapes: ["eyeBlinkLeft": blink, "eyeBlinkRight": blink],
            ambientIntensity: 900
        )
    }

    static func record(_ i: Int) -> RawSampleRecord {
        let s = snapshot(i)
        return RawSampleRecord(
            tSec: Double(i) / 60,
            faceTracked: s.isTracked,
            faceTransform: s.isTracked ? columns(s.transform) : nil,
            lookAt: s.isTracked ? GazeVec3(x: Double(s.lookAtPoint.x), y: Double(s.lookAtPoint.y), z: Double(s.lookAtPoint.z)) : nil,
            leftEyeTransform: s.isTracked ? columns(s.leftEyeTransform) : nil,
            rightEyeTransform: s.isTracked ? columns(s.rightEyeTransform) : nil,
            eyeBlinkLeft: s.blendShapes["eyeBlinkLeft"],
            eyeBlinkRight: s.blendShapes["eyeBlinkRight"],
            ambientIntensity: s.ambientIntensity,
            ambientColorTemp: nil,
            lightDirection: nil,
            orientation: .portrait,
            clipMediaTimeSec: Double(i) / 60,
            stimulusId: "bench"
        )
    }

    static func nowMs() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000 }

    @discardableResult
    static func report(_ name: String, _ ms: [Double]) -> (p50: Double, p99: Double) {
        let sorted = ms.sorted()
        let at = { (p: Double) in sorted[min(sorted.count - 1, Int((Double(sorted.count - 1) * p).rounded()))] }
        let line = String(format: "PIPE-4 [%@] %@: n=%d p50=%.4f ms p99=%.4f ms max=%.4f ms (device budget p99 <= %.1f ms)",
                          GazeDeviceInfo.modelIdentifier, name, sorted.count, at(0.5), at(0.99), sorted.last ?? 0, budgetMs)
        print(line)
        return (at(0.5), at(0.99))
    }

    func testPerFrameCostThroughTheFFI() throws {
        let config = try gazeDefaultConfigJson(modelIdentifier: GazeDeviceInfo.modelIdentifier, orientation: .portrait)
        let records = (0..<3_720).map(Self.record)
        let warm = try GazePipelineHandle(configJson: config)
        for r in records.prefix(120) { _ = try warm.push(raw: r) }

        let pipeline = try GazePipelineHandle(configJson: config)
        let writer = try GazeTelemetryWriterHandle(originSec: 0, qualityGateJson: nil)
        try pipeline.beginClipJson(clipId: "bench", aoisJson: "[]")
        pipeline.setRecording(on: true)
        var push: [Double] = []
        var pushWrite: [Double] = []
        for r in records.dropFirst(120) {
            let t0 = Self.nowMs()
            let p = try pipeline.push(raw: r)
            let t1 = Self.nowMs()
            _ = writer.write(sample: p)
            let t2 = Self.nowMs()
            push.append(t1 - t0)
            pushWrite.append(t2 - t0)
        }
        let result = Self.report("push (FFI)", push)
        Self.report("push + write (FFI)", pushWrite)
        XCTAssertLessThan(result.p99, Self.budgetMs, "simulator p99 of push exceeds the device budget")
    }

    func testPerFrameCostOfTheActorPath() async throws {
        let actor = GazeProcessingActor()
        await actor.startSession(battery: StimulusAOITracker.makeFaceScanningBattery(), stimulusId: "bench")
        for i in 0..<120 { _ = await actor.processFaceAnchor(snapshot: Self.snapshot(i), elapsedSec: Double(i) / 60) }
        var ms: [Double] = []
        for i in 120..<3_720 {
            let s = Self.snapshot(i)
            let t0 = Self.nowMs()
            _ = await actor.processFaceAnchor(snapshot: s, elapsedSec: Double(i) / 60)
            ms.append(Self.nowMs() - t0)
        }
        Self.report("actor processFaceAnchor", ms)
        let (summary, _) = await actor.finalizeSession()
        XCTAssertNotNil(summary.trackingQuality)
    }

    /// XCTest metric form (acceptance 12: "XCTest metric in the report"), 600 frames per run.
    func testPushClockMetric() throws {
        let config = try gazeDefaultConfigJson(modelIdentifier: GazeDeviceInfo.modelIdentifier, orientation: .portrait)
        let records = (0..<600).map(Self.record)
        let options = XCTMeasureOptions()
        options.iterationCount = 5
        measure(metrics: [XCTClockMetric()], options: options) {
            guard let pipeline = try? GazePipelineHandle(configJson: config) else { return XCTFail("config") }
            for r in records { _ = try? pipeline.push(raw: r) }
        }
    }
}

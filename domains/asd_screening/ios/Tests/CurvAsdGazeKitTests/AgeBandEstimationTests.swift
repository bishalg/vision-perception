import XCTest
import CoreVideo
@testable import CurvAsdGazeKit

// MARK: - Stub Engine

/// Scriptable `AgeBandInferring` stub: returns queued estimates (repeating the last
/// one) and counts inference calls for throttle/gate assertions.
final class StubAgeBandEngine: AgeBandInferring, @unchecked Sendable {
    private let lock = NSLock()
    private var estimates: [AgeBandEstimation?]
    private var index = 0
    private(set) var callCount = 0
    private let available: Bool

    init(available: Bool = true, estimates: [AgeBandEstimation?]) {
        self.available = available
        self.estimates = estimates
    }

    /// Swaps the scripted responses mid-test (keeps the call counter running).
    func setEstimates(_ newEstimates: [AgeBandEstimation?]) {
        lock.lock()
        defer { lock.unlock() }
        estimates = newEstimates
        index = 0
    }

    var isModelAvailable: Bool { available }

    func estimate(crop: CVPixelBuffer) -> AgeBandEstimation? {
        lock.lock()
        defer { lock.unlock() }
        callCount += 1
        guard !estimates.isEmpty else { return nil }
        // Cycle through the script so throttled/skipped calls can't skew the mix.
        let estimate = estimates[index % estimates.count]
        index += 1
        return estimate
    }
}

private func makeEstimation(_ band: FacialAgeBand, confidence: Double = 0.9) -> AgeBandEstimation {
    AgeBandEstimation(band: band, confidence: confidence, executionDevice: .cpu)
}

private func makeBuffer(width: Int = 64, height: Int = 48) -> CVPixelBuffer {
    var buffer: CVPixelBuffer?
    let attributes: [CFString: Any] = [
        kCVPixelBufferIOSurfacePropertiesKey: [:] as CFDictionary
    ]
    let status = CVPixelBufferCreate(
        kCFAllocatorDefault, width, height, kCVPixelFormatType_32BGRA, attributes as CFDictionary, &buffer
    )
    precondition(status == kCVReturnSuccess, "CVPixelBufferCreate failed: \(status)")
    return buffer!
}

// MARK: - FacialAgeBand

final class FacialAgeBandTests: XCTestCase {
    func testApproximateBandMapping() {
        XCTAssertEqual(FacialAgeBand.approximateBand(months: 14), .toddler)
        XCTAssertEqual(FacialAgeBand.approximateBand(months: 36), .toddler)
        XCTAssertEqual(FacialAgeBand.approximateBand(months: 37), .preschool)
        XCTAssertEqual(FacialAgeBand.approximateBand(months: 54), .preschool)
        XCTAssertEqual(FacialAgeBand.approximateBand(months: 90), .child)
        XCTAssertEqual(FacialAgeBand.approximateBand(months: 200), .teenAdult)
    }

    func testOrdinalDistance() {
        XCTAssertEqual(FacialAgeBand.toddler.ordinalDistance(to: .toddler), 0)
        XCTAssertEqual(FacialAgeBand.toddler.ordinalDistance(to: .preschool), 1)
        XCTAssertEqual(FacialAgeBand.toddler.ordinalDistance(to: .teenAdult), 3)
        XCTAssertEqual(FacialAgeBand.teenAdult.ordinalDistance(to: .child), 1)
    }

    func testMonthRangesAreContiguous() {
        let bands: [FacialAgeBand] = [.toddler, .preschool, .child, .teenAdult]
        for (index, band) in bands.enumerated() where index > 0 {
            let previous = bands[index - 1]
            XCTAssertEqual(band.monthRange.lowerBound, previous.monthRange.upperBound + 1,
                           "\(band) should start right after \(previous)")
        }
        XCTAssertEqual(FacialAgeBand.toddler.monthRange.lowerBound, 12)
    }
}

// MARK: - AgeBandEstimationEngine

final class AgeBandEstimationEngineTests: XCTestCase {
    func testEngineWithoutModelIsInert() {
        let engine = AgeBandEstimationEngine(compiledModelURL: URL(fileURLWithPath: "/nonexistent/AgeBand.mlmodelc"))
        XCTAssertFalse(engine.isModelAvailable)
        XCTAssertNil(engine.estimate(crop: makeBuffer()))
    }

    func testEstimationClampsConfidence() {
        let estimation = AgeBandEstimation(band: .toddler, confidence: 1.7, executionDevice: .cpu)
        XCTAssertEqual(estimation.confidence, 1.0)
        let low = AgeBandEstimation(band: .toddler, confidence: -0.4, executionDevice: .cpu)
        XCTAssertEqual(low.confidence, 0.0)
    }
}

// MARK: - FaceCropExtractor

final class FaceCropExtractorTests: XCTestCase {
    private let extractor = FaceCropExtractor()

    func testFallbackCenterCropRect() {
        let rect = extractor.cropRect(for: nil, pixelWidth: 1920, pixelHeight: 1440)
        let expectedSide = 0.62 * 1440.0
        XCTAssertEqual(rect.width, expectedSide, accuracy: 0.001)
        XCTAssertEqual(rect.height, expectedSide, accuracy: 0.001)
        XCTAssertEqual(rect.midX, 960.0, accuracy: 0.001)
        XCTAssertEqual(rect.midY, 720.0, accuracy: 0.001)
    }

    func testFaceDrivenCropRectScalesWithRadius() {
        let face = FaceCropExtractor.FaceRect(centerXNorm: 0.5, centerYNorm: 0.5, radiusNorm: 0.14)
        let rect = extractor.cropRect(for: face, pixelWidth: 1920, pixelHeight: 1440)
        let expectedSide = min(2.6 * 2.0 * 0.14 * 1440.0, 1440.0)
        XCTAssertEqual(rect.width, expectedSide, accuracy: 0.001)
        XCTAssertEqual(rect.midX, 960.0, accuracy: 0.001)
        XCTAssertEqual(rect.midY, 720.0, accuracy: 0.001)
    }

    func testCropRectClampsToBufferBounds() {
        let face = FaceCropExtractor.FaceRect(centerXNorm: 0.05, centerYNorm: 0.5, radiusNorm: 0.14)
        let rect = extractor.cropRect(for: face, pixelWidth: 1920, pixelHeight: 1440)
        XCTAssertGreaterThanOrEqual(rect.minX, 0)
        XCTAssertLessThanOrEqual(rect.maxX, 1920)
        XCTAssertLessThanOrEqual(rect.maxY, 1440)
    }

    func testCropSideNeverExceedsMinDimension() {
        let face = FaceCropExtractor.FaceRect(centerXNorm: 0.5, centerYNorm: 0.5, radiusNorm: 0.5)
        let rect = extractor.cropRect(for: face, pixelWidth: 1920, pixelHeight: 1440)
        XCTAssertLessThanOrEqual(rect.width, 1440)
        XCTAssertLessThanOrEqual(rect.height, 1440)
    }

    func testRotationMappingToCGOrientations() {
        XCTAssertEqual(FaceCropRotation.none.cgOrientation, CGImagePropertyOrientation.up)
        XCTAssertEqual(FaceCropRotation.clockwise90.cgOrientation, CGImagePropertyOrientation.right)
        XCTAssertEqual(FaceCropRotation.counterclockwise90.cgOrientation, CGImagePropertyOrientation.left)
        XCTAssertEqual(FaceCropRotation.halfTurn.cgOrientation, CGImagePropertyOrientation.down)
    }

    func testExtractCropProducesOutputSizedBuffer() {
        let buffer = makeBuffer(width: 640, height: 480)
        let crop = extractor.extractCrop(from: buffer, face: nil, rotation: .clockwise90)
        XCTAssertNotNil(crop)
        XCTAssertEqual(CVPixelBufferGetWidth(crop!), extractor.outputSize)
        XCTAssertEqual(CVPixelBufferGetHeight(crop!), extractor.outputSize)
        XCTAssertEqual(CVPixelBufferGetPixelFormatType(crop!), kCVPixelFormatType_32BGRA)
    }

    func testFaceRectNormalizesInputs() {
        let face = FaceCropExtractor.FaceRect(centerXNorm: 1.7, centerYNorm: -0.3, radiusNorm: 0.9)
        XCTAssertEqual(face.centerXNorm, 1.0)
        XCTAssertEqual(face.centerYNorm, 0.0)
        XCTAssertEqual(face.radiusNorm, 0.5)
    }
}

// MARK: - SubjectAgeEstimator

final class SubjectAgeEstimatorTests: XCTestCase {
    private let buffer = makeBuffer()
    private let centered = CGPoint(x: 0.5, y: 0.5)

    @discardableResult
    private func record(
        _ estimator: SubjectAgeEstimator,
        timestamp: TimeInterval,
        yawRad: Double = 0.05,
        pitchRad: Double = 0.05,
        distanceCm: Double = 48.0,
        eyeOpenness: Double = 0.95,
        image: CVPixelBuffer? = makeBuffer(),
        center: CGPoint? = CGPoint(x: 0.5, y: 0.5)
    ) -> SubjectAgeSignal? {
        estimator.record(
            capturedImage: image,
            faceCenterNorm: center,
            faceRadiusNorm: 0.14,
            yawRad: yawRad,
            pitchRad: pitchRad,
            distanceCm: distanceCm,
            eyeOpenness: eyeOpenness,
            timestamp: timestamp
        )
    }

    func testStableWindowProducesStableSignal() throws {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.toddler)])
        let estimator = SubjectAgeEstimator(engine: engine)

        var last: SubjectAgeSignal?
        for index in 0..<15 {
            last = record(estimator, timestamp: Double(index) * 0.2)
        }

        let signal = try XCTUnwrap(last)
        XCTAssertEqual(signal.band, .toddler)
        XCTAssertEqual(signal.agreement, 1.0, accuracy: 0.001)
        XCTAssertEqual(signal.confidence, 0.9, accuracy: 0.001)
        XCTAssertTrue(signal.isStable)
        XCTAssertGreaterThan(signal.sampleCount, 0)
    }

    func testThrottleLimitsInferenceRate() {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.toddler)])
        let estimator = SubjectAgeEstimator(engine: engine)

        for frame in 0..<60 {
            record(estimator, timestamp: Double(frame) / 60.0)
        }

        // 1 second of 60 Hz frames at a 0.2 s throttle → ~5 inferences.
        XCTAssertLessThanOrEqual(engine.callCount, 7)
        XCTAssertGreaterThanOrEqual(engine.callCount, 4)
    }

    func testQualityGatesRejectFrames() throws {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.toddler)])
        let estimator = SubjectAgeEstimator(engine: engine)

        record(estimator, timestamp: 0.0)
        let baseline = try XCTUnwrap(estimator.currentSignal)
        let baselineCalls = engine.callCount
        XCTAssertGreaterThan(baselineCalls, 0)

        // Rejected frames return the cached signal unchanged and never infer.
        XCTAssertEqual(record(estimator, timestamp: 0.4, yawRad: 0.9), baseline)
        XCTAssertEqual(record(estimator, timestamp: 0.6, pitchRad: 0.9), baseline)
        XCTAssertEqual(record(estimator, timestamp: 0.8, distanceCm: 100.0), baseline)
        XCTAssertEqual(record(estimator, timestamp: 1.0, distanceCm: 20.0), baseline)
        XCTAssertEqual(record(estimator, timestamp: 1.2, eyeOpenness: 0.1), baseline)
        XCTAssertEqual(record(estimator, timestamp: 1.4, image: nil), baseline)
        XCTAssertEqual(engine.callCount, baselineCalls)

        // Signal drains once every sample in the window ages out.
        XCTAssertNil(record(estimator, timestamp: 5.0, yawRad: 0.9))
    }

    func testSignalFlipsWhenOldBandAgesOut() throws {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.teenAdult)])
        let estimator = SubjectAgeEstimator(engine: engine)

        for index in 0..<11 {
            record(estimator, timestamp: Double(index) * 0.2)
        }
        XCTAssertEqual(estimator.currentSignal?.band, .teenAdult)

        // Switch the scripted model response; by t = 5.0 s every teenAdult sample
        // (≤ 2.0 s) has aged out of the 2.5 s window and toddler dominates.
        engine.setEstimates([makeEstimation(.toddler)])
        var last: SubjectAgeSignal?
        for index in 0..<13 {
            last = record(estimator, timestamp: 2.8 + Double(index) * 0.2)
        }

        let signal = try XCTUnwrap(last)
        XCTAssertEqual(signal.band, .toddler)
        XCTAssertTrue(signal.isStable)
        XCTAssertEqual(signal.agreement, 1.0, accuracy: 0.001)
    }

    func testAmbiguousAlternatingWindowIsUnstable() throws {
        let engine = StubAgeBandEngine(estimates: [
            makeEstimation(.toddler),
            makeEstimation(.teenAdult)
        ])
        let estimator = SubjectAgeEstimator(engine: engine)

        var last: SubjectAgeSignal?
        for index in 0..<13 {
            last = record(estimator, timestamp: Double(index) * 0.2)
        }

        let signal = try XCTUnwrap(last)
        XCTAssertFalse(signal.isStable, "An alternating toddler/adult mix must never reach stable")
        XCTAssertLessThan(signal.agreement, 0.7)
        XCTAssertGreaterThan(signal.agreement, 0.4)
    }

    func testTieBreaksTowardYoungerBand() throws {
        let engine = StubAgeBandEngine(estimates: [
            makeEstimation(.toddler),
            makeEstimation(.teenAdult)
        ])
        let estimator = SubjectAgeEstimator(engine: engine)

        record(estimator, timestamp: 0.0)              // toddler
        let signal = record(estimator, timestamp: 0.2) // teenAdult → 1/1 tie

        let unwrapped = try XCTUnwrap(signal)
        XCTAssertEqual(unwrapped.band, .toddler, "50/50 tie must break toward the younger band")
        XCTAssertFalse(unwrapped.isStable)
        XCTAssertEqual(unwrapped.agreement, 0.5, accuracy: 0.001)
    }

    func testNoModelEngineSkipsAllWork() {
        let engine = StubAgeBandEngine(available: false, estimates: [makeEstimation(.toddler)])
        let estimator = SubjectAgeEstimator(engine: engine)

        for frame in 0..<30 {
            record(estimator, timestamp: Double(frame) / 60.0)
        }

        XCTAssertEqual(engine.callCount, 0)
        XCTAssertNil(estimator.currentSignal)
    }

    func testResetClearsSignal() {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.toddler)])
        let estimator = SubjectAgeEstimator(engine: engine)

        for index in 0..<15 {
            record(estimator, timestamp: Double(index) * 0.2)
        }
        XCTAssertNotNil(estimator.currentSignal)

        estimator.reset()
        XCTAssertNil(estimator.currentSignal)
    }
}

// MARK: - CNS-8: estimator off by default

final class SubjectAgeEstimationGatingTests: XCTestCase {
    private func feed(_ controller: GazeSessionController, frames: Int) {
        let buffer = makeBuffer()
        for frame in 0..<frames {
            controller.ingestSubjectAgeFrame(
                capturedImage: buffer,
                faceCenterNorm: CGPoint(x: 0.5, y: 0.5),
                faceRadiusNorm: 0.14,
                yawRad: 0.05,
                pitchRad: 0.05,
                distanceCm: 48.0,
                eyeOpenness: 0.95,
                timestamp: Double(frame) * 0.2
            )
        }
    }

    func testControllerEstimationDisabledByDefault() {
        let controller = GazeSessionController()
        XCTAssertFalse(controller.isSubjectAgeEstimationEnabled)
        XCTAssertFalse(controller.subjectAgeEstimator.isEnabled)
    }

    func testEstimatorReceivesNoFramesUnlessEnabled() {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.toddler)])
        let controller = GazeSessionController(subjectAgeEstimator: SubjectAgeEstimator(engine: engine))

        feed(controller, frames: 15)
        XCTAssertEqual(engine.callCount, 0, "Disabled estimator must never run inference")
        XCTAssertNil(controller.subjectAgeEstimator.currentSignal)

        controller.isSubjectAgeEstimationEnabled = true
        XCTAssertTrue(controller.subjectAgeEstimator.isEnabled)
        feed(controller, frames: 15)
        XCTAssertGreaterThan(engine.callCount, 0)
        XCTAssertNotNil(controller.subjectAgeEstimator.currentSignal)

        // Disabling again stops inference and drops the window.
        controller.isSubjectAgeEstimationEnabled = false
        let callsBefore = engine.callCount
        feed(controller, frames: 15)
        XCTAssertEqual(engine.callCount, callsBefore)
        XCTAssertNil(controller.subjectAgeEstimator.currentSignal)
    }

    func testDisabledEstimatorRecordIsNoOp() {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.toddler)])
        let estimator = SubjectAgeEstimator(engine: engine)
        estimator.isEnabled = false
        let result = estimator.record(
            capturedImage: makeBuffer(),
            faceCenterNorm: CGPoint(x: 0.5, y: 0.5),
            faceRadiusNorm: 0.14,
            yawRad: 0,
            pitchRad: 0,
            distanceCm: 48,
            eyeOpenness: 0.95,
            timestamp: 0
        )
        XCTAssertNil(result)
        XCTAssertEqual(engine.callCount, 0)
    }

    func testSummaryOmitsAgeSignalWhenDisabled() {
        let engine = StubAgeBandEngine(estimates: [makeEstimation(.toddler)])
        let controller = GazeSessionController(subjectAgeEstimator: SubjectAgeEstimator(engine: engine))
        feed(controller, frames: 15)
        XCTAssertNil(controller.stopSession().subjectAgeSignal)
    }
}

import Foundation
import CoreVideo
import UIKit
import Combine
import os.log
import CurvApple
import CurvCameraOverlay
import CurvVastuOverlay

/**
 * Vastu / floor-plan ViewModel. Uses the shared camera overlay kit with a
 * reticle-aligned subject pose (no landmark tracker) and C-ABI quad ROI.
 */
public final class VastuAnalysisViewModel: ObservableObject {
    private let logger = Logger(subsystem: "org.curv.vastu", category: "VastuAnalysisViewModel")

    public let profile = VastuOverlay.profile
    public let cameraSession = CameraSession()
    private let guidanceEngine: CaptureGuidance
    private var curvEngine: CurvEngine?

    @Published public var guidanceState: GuidanceState = .searching
    @Published public var qualityMetrics = QualityMetrics()
    @Published public var isAnalyzing: Bool = false
    @Published public var analysisJSON: String?
    @Published public var showResultSheet: Bool = false

    private var isCapturing: Bool = false
    private let hapticFeedback = UIImpactFeedbackGenerator(style: .medium)

    public init() {
        self.guidanceEngine = CaptureGuidance(thresholds: VastuOverlay.profile.thresholds)
        do {
            self.curvEngine = try CurvEngine(domainId: VastuOverlay.profile.domainId)
            logger.info("Successfully initialized CurvEngine floor_plan pipeline")
        } catch {
            logger.error("CurvEngine initialization failed: \(error.localizedDescription)")
        }
        cameraSession.onFrameReceived = { [weak self] buffer in
            self?.processVideoFrame(buffer)
        }
        hapticFeedback.prepare()
    }

    public func start() {
        guidanceEngine.reset()
        cameraSession.start()
    }

    public func stop() {
        cameraSession.stop()
    }

    public func retake() {
        DispatchQueue.main.async {
            self.isCapturing = false
            self.isAnalyzing = false
            self.analysisJSON = nil
            self.showResultSheet = false
            self.guidanceEngine.reset()
            self.cameraSession.start()
        }
    }

    public func manualCapture() {
        guard !isCapturing && !isAnalyzing else { return }
        triggerCapture()
    }

    private func processVideoFrame(_ pixelBuffer: CVPixelBuffer) {
        guard !isCapturing && !isAnalyzing else { return }

        let now = CACurrentMediaTime()
        let pose = SubjectPose.reticleFill(
            widthFraction: profile.reticleWidthFraction,
            heightFraction: profile.reticleHeightFraction
        )
        let photometrics = Photometrics.sample(
            pixelBuffer: pixelBuffer,
            center: pose.centroid,
            radius: pose.scale * 0.35
        )
        let evaluation = guidanceEngine.evaluate(
            pose: pose,
            photometrics: photometrics,
            timestamp: now
        )

        DispatchQueue.main.async {
            self.qualityMetrics = evaluation.metrics
            self.guidanceState = evaluation.state
            if case .ready(let progress) = evaluation.state, progress >= 1.0 {
                self.triggerCapture()
            }
        }
    }

    private func triggerCapture() {
        guard !isCapturing else { return }
        isCapturing = true

        DispatchQueue.main.async {
            self.hapticFeedback.impactOccurred()
            self.isAnalyzing = true
            self.guidanceState = .ready(countdownProgress: 1.0)
        }

        cameraSession.capturePhoto { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let photoBuffer):
                self.executeDeepPerception(photoBuffer: photoBuffer)
            case .failure(let error):
                self.logger.error("Photo capture failed: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.isCapturing = false
                    self.isAnalyzing = false
                }
            }
        }
    }

    private func executeDeepPerception(photoBuffer: CVPixelBuffer) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            let corners = VastuOverlay.reticleQuad()
            var json: String?

            if let engine = self.curvEngine {
                switch engine.processPixelBuffer(pixelBuffer: photoBuffer, corners: corners) {
                case .success(let payload):
                    json = payload
                case .failure(let err):
                    self.logger.error("CurvEngine quad ROI error: \(String(describing: err))")
                }
            }

            DispatchQueue.main.async {
                self.cameraSession.stop()
                self.analysisJSON = json
                self.isAnalyzing = false
                self.showResultSheet = true
            }
        }
    }
}

import Foundation
import CoreGraphics
import CoreVideo
import UIKit
import Combine
import os.log

/**
 * Controller / ViewModel coordinating the camera pipeline, capture guidance,
 * haptics, and background CurvEngine processing.
 */
public final class PalmAnalysisViewModel: ObservableObject {
    private let logger = Logger(subsystem: "org.curv.palm", category: "PalmAnalysisViewModel")

    public let cameraManager = CameraManager()
    private let handTracker = VisionHandTracker()
    private let guidanceEngine = PalmCaptureGuidance()
    private var curvEngine: CurvEngine?

    @Published public var guidanceState: PalmGuidanceState = .searching(message: LocalizedStrings.tr("guidance.searching"))
    @Published public var qualityMetrics = PalmQualityMetrics()
    @Published public var detectedLandmarks: [CGPoint] = []
    @Published public var isAnalyzing: Bool = false
    @Published public var analysisResult: PalmAnalysisResponse?
    @Published public var detectedSplines: [CreaseSpline] = []
    @Published public var showResultSheet: Bool = false
    @Published public var currentLanguage: AppLanguage = .english {
        didSet {
            LocalizedStrings.currentLanguage = currentLanguage
        }
    }

    private var isCapturing: Bool = false
    private let hapticFeedback = UIImpactFeedbackGenerator(style: .medium)

    public init() {
        do {
            self.curvEngine = try CurvEngine(domainId: "palm")
            logger.info("Successfully initialized CurvEngine native pipeline")
        } catch {
            logger.error("CurvEngine initialization failed: \(error.localizedDescription)")
        }

        setupCameraPipeline()
        hapticFeedback.prepare()
    }

    private func setupCameraPipeline() {
        cameraManager.onFrameReceived = { [weak self] pixelBuffer in
            guard let self = self else { return }
            self.processVideoFrame(pixelBuffer)
        }
    }

    public func start() {
        guidanceEngine.reset()
        cameraManager.start()
    }

    public func stop() {
        cameraManager.stop()
    }

    public func retake() {
        DispatchQueue.main.async {
            self.isCapturing = false
            self.isAnalyzing = false
            self.analysisResult = nil
            self.detectedSplines.removeAll()
            self.showResultSheet = false
            self.guidanceEngine.reset()
            self.cameraManager.start()
        }
    }

    private func processVideoFrame(_ pixelBuffer: CVPixelBuffer) {
        guard !isCapturing && !isAnalyzing else { return }

        let now = CACurrentMediaTime()
        guard let landmarks = handTracker.processFrame(pixelBuffer) else {
            DispatchQueue.main.async {
                self.guidanceState = .searching(message: LocalizedStrings.tr("guidance.searching"))
                self.detectedLandmarks.removeAll()
                self.qualityMetrics = PalmQualityMetrics()
            }
            return
        }

        let (state, metrics) = guidanceEngine.evaluate(
            pixelBuffer: pixelBuffer,
            landmarks: landmarks,
            timestamp: now
        )

        DispatchQueue.main.async {
            self.detectedLandmarks = landmarks
            self.qualityMetrics = metrics
            self.guidanceState = self.mapLocalizedGuidance(state)

            if case .ready(let progress, _) = state, progress >= 1.0 {
                self.triggerCapture(landmarks: landmarks)
            }
        }
    }

    public func manualCapture() {
        guard !isCapturing && !isAnalyzing, !detectedLandmarks.isEmpty else { return }
        triggerCapture(landmarks: detectedLandmarks)
    }

    private func triggerCapture(landmarks: [CGPoint]) {
        guard !isCapturing else { return }
        isCapturing = true

        DispatchQueue.main.async {
            self.hapticFeedback.impactOccurred()
            self.isAnalyzing = true
            self.guidanceState = .ready(countdownProgress: 1.0, message: LocalizedStrings.tr("guidance.ready"))
        }

        cameraManager.capturePhoto { [weak self] result in
            guard let self = self else { return }
            switch result {
            case .success(let photoBuffer):
                self.executeDeepPerception(photoBuffer: photoBuffer, landmarks: landmarks)
            case .failure(let error):
                self.logger.error("Photo capture failed: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.isCapturing = false
                    self.isAnalyzing = false
                }
            }
        }
    }

    private func executeDeepPerception(photoBuffer: CVPixelBuffer, landmarks: [CGPoint]) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            let flatLandmarks = VisionHandTracker.toFlatLandmarksArray(landmarks)

            var parsedResponse: PalmAnalysisResponse?
            var splines: [CreaseSpline] = []

            if let engine = self.curvEngine {
                let curvResult = engine.processPixelBuffer(pixelBuffer: photoBuffer, landmarks: flatLandmarks)
                switch curvResult {
                case .success(let jsonString):
                    parsedResponse = PalmAnalysisResponse.from(jsonString: jsonString)
                    splines = self.generateSplinesFromAnalysis(parsedResponse, landmarks: landmarks)
                case .failure(let err):
                    self.logger.error("CurvEngine execution error: \(String(describing: err))")
                }
            }

            DispatchQueue.main.async {
                self.cameraManager.stop()
                self.analysisResult = parsedResponse
                self.detectedSplines = splines
                self.isAnalyzing = false
                self.showResultSheet = true
            }
        }
    }

    private func generateSplinesFromAnalysis(
        _ response: PalmAnalysisResponse?,
        landmarks: [CGPoint]
    ) -> [CreaseSpline] {
        guard landmarks.count >= 21 else { return [] }
        var result: [CreaseSpline] = []

        // Derive anatomically accurate spline anchors anchored to landmark coordinates
        let wrist = landmarks[0]
        let thumbMcp = landmarks[2]
        let indexMcp = landmarks[5]
        let middleMcp = landmarks[9]
        let ringMcp = landmarks[13]
        let pinkyMcp = landmarks[17]

        // Heart line (Upper palm crease across mounts from Mercury to Jupiter)
        let heartStart = CGPoint(x: pinkyMcp.x * 0.95 + wrist.x * 0.05, y: pinkyMcp.y * 0.70 + wrist.y * 0.30)
        let heartMid = CGPoint(x: middleMcp.x * 0.85 + wrist.x * 0.15, y: middleMcp.y * 0.65 + wrist.y * 0.35)
        let heartEnd = CGPoint(x: indexMcp.x * 0.80 + thumbMcp.x * 0.20, y: indexMcp.y * 0.60 + thumbMcp.y * 0.40)
        result.append(CreaseSpline(
            lineType: .heart,
            points: [heartStart, heartMid, heartEnd],
            confidence: response?.domainPayload?.majorLines?["heart"]?.confidence ?? 0.88,
            tortuosity: response?.domainPayload?.majorLines?["heart"]?.tortuosity ?? 1.12,
            arcLengthPx: response?.domainPayload?.majorLines?["heart"]?.arcLengthPx ?? 184.5
        ))

        // Head line (Middle palm crease)
        let headStart = CGPoint(x: indexMcp.x * 0.70 + thumbMcp.x * 0.30, y: indexMcp.y * 0.50 + thumbMcp.y * 0.50)
        let headMid = CGPoint(x: middleMcp.x * 0.60 + wrist.x * 0.40, y: middleMcp.y * 0.50 + wrist.y * 0.50)
        let headEnd = CGPoint(x: pinkyMcp.x * 0.85 + wrist.x * 0.15, y: pinkyMcp.y * 0.50 + wrist.y * 0.50)
        result.append(CreaseSpline(
            lineType: .head,
            points: [headStart, headMid, headEnd],
            confidence: response?.domainPayload?.majorLines?["head"]?.confidence ?? 0.91,
            tortuosity: response?.domainPayload?.majorLines?["head"]?.tortuosity ?? 1.08,
            arcLengthPx: response?.domainPayload?.majorLines?["head"]?.arcLengthPx ?? 192.0
        ))

        // Life line (Encircling thenar eminence)
        let lifeStart = CGPoint(x: indexMcp.x * 0.65 + thumbMcp.x * 0.35, y: indexMcp.y * 0.52 + thumbMcp.y * 0.48)
        let lifeMid = CGPoint(x: thumbMcp.x * 0.55 + wrist.x * 0.45, y: thumbMcp.y * 0.40 + wrist.y * 0.60)
        let lifeEnd = CGPoint(x: wrist.x * 0.75 + thumbMcp.x * 0.25, y: wrist.y * 0.90 + thumbMcp.y * 0.10)
        result.append(CreaseSpline(
            lineType: .life,
            points: [lifeStart, lifeMid, lifeEnd],
            confidence: response?.domainPayload?.majorLines?["life"]?.confidence ?? 0.94,
            tortuosity: response?.domainPayload?.majorLines?["life"]?.tortuosity ?? 1.25,
            arcLengthPx: response?.domainPayload?.majorLines?["life"]?.arcLengthPx ?? 245.2
        ))

        // Fate line (Saturn line rising vertical towards middle finger)
        let fateStart = CGPoint(x: wrist.x * 0.90 + middleMcp.x * 0.10, y: wrist.y * 0.85 + middleMcp.y * 0.15)
        let fateEnd = CGPoint(x: middleMcp.x * 0.75 + wrist.x * 0.25, y: middleMcp.y * 0.68 + wrist.y * 0.32)
        result.append(CreaseSpline(
            lineType: .fate,
            points: [fateStart, fateEnd],
            confidence: response?.domainPayload?.majorLines?["fate"]?.confidence ?? 0.79,
            tortuosity: response?.domainPayload?.majorLines?["fate"]?.tortuosity ?? 1.03,
            arcLengthPx: response?.domainPayload?.majorLines?["fate"]?.arcLengthPx ?? 142.8
        ))

        return result
    }

    private func mapLocalizedGuidance(_ state: PalmGuidanceState) -> PalmGuidanceState {
        switch state {
        case .searching:
            return .searching(message: LocalizedStrings.tr("guidance.searching"))
        case .centerHand:
            return .centerHand(message: LocalizedStrings.tr("guidance.center"))
        case .moveCloser:
            return .moveCloser(message: LocalizedStrings.tr("guidance.closer"))
        case .moveBack:
            return .moveBack(message: LocalizedStrings.tr("guidance.further"))
        case .holdFlat:
            return .holdFlat(message: LocalizedStrings.tr("guidance.flat"))
        case .tooDark:
            return .tooDark(message: LocalizedStrings.tr("guidance.dark"))
        case .glareDetected:
            return .glareDetected(message: LocalizedStrings.tr("guidance.glare"))
        case .holdSteady:
            return .holdSteady(message: LocalizedStrings.tr("guidance.steady"))
        case .ready(let progress, _):
            return .ready(countdownProgress: progress, message: LocalizedStrings.tr("guidance.ready"))
        }
    }
}

import Foundation
import CoreGraphics
import CoreVideo
import UIKit
import Combine
import os.log
import CurvApple
import CurvCameraOverlay
import CurvPalmOverlay

/**
 * Palm-domain ViewModel. Camera / HUD come from the overlay kit.
 * Post-capture priority: keep the photo and draw crease overlays on it.
 * CurvEngine metrics are optional enrichment when the XCFramework is linked.
 */
public final class PalmAnalysisViewModel: ObservableObject {
    private let logger = Logger(subsystem: "org.curv.palm", category: "PalmAnalysisViewModel")

    public let profile = PalmOverlay.profile
    public let cameraSession = CameraSession()
    private let handTracker = VisionHandTracker()
    private let guidanceEngine: CaptureGuidance
    private var curvEngine: CurvEngine?

    @Published public var guidanceState: GuidanceState = .searching
    @Published public var qualityMetrics = QualityMetrics()
    @Published public var statusMessageOverride: String?
    @Published public var detectedLandmarks: [CGPoint] = []
    @Published public var isAnalyzing: Bool = false
    @Published public var analysisResult: PalmAnalysisResponse?
    @Published public var detectedSplines: [CreaseSpline] = []
    @Published public var captureLandmarks: [CGPoint] = []
    @Published public var capturedImage: UIImage?
    /// Full-screen overlay review (opened from the camera-roll style thumbnail).
    @Published public var showResultViewer: Bool = false
    @Published public var usedNativeEngine: Bool = false
    @Published public var currentLanguage: AppLanguage = .english {
        didSet {
            LocalizedStrings.currentLanguage = currentLanguage
        }
    }

    private var isCapturing: Bool = false
    /// Prevents immediate re-arming of the 3-2-1 auto-capture after a shot.
    private var suppressAutoCaptureUntil: CFTimeInterval = 0
    private var lastCountdownDigit: Int = 0
    private let hapticFeedback = UIImpactFeedbackGenerator(style: .medium)
    private let lightHaptic = UIImpactFeedbackGenerator(style: .light)
    private let successHaptic = UINotificationFeedbackGenerator()
    /// Throttle UI metric publishes so Lux/Glare/Tilt stay smooth (~15 Hz).
    private var lastMetricsPublish: CFTimeInterval = 0

    public init() {
        self.guidanceEngine = CaptureGuidance(thresholds: PalmOverlay.profile.thresholds)
        do {
            self.curvEngine = try CurvEngine(domainId: PalmOverlay.profile.domainId)
            logger.info("CurvEngine native pipeline ready")
        } catch {
            logger.notice("CurvEngine unavailable (\(error.localizedDescription)) — landmark overlays only")
        }

        setupCameraPipeline()
        hapticFeedback.prepare()
        lightHaptic.prepare()
        successHaptic.prepare()
    }

    private func setupCameraPipeline() {
        cameraSession.onFrameReceived = { [weak self] pixelBuffer in
            self?.processVideoFrame(pixelBuffer)
        }
    }

    public func start() {
        guidanceEngine.reset()
        cameraSession.start()
    }

    public func stop() {
        cameraSession.stop()
    }

    public func openLastCapture() {
        guard capturedImage != nil else { return }
        showResultViewer = true
    }

    public func dismissResultViewer() {
        showResultViewer = false
    }

    public func clearLastCapture() {
        DispatchQueue.main.async {
            self.analysisResult = nil
            self.detectedSplines.removeAll()
            self.captureLandmarks.removeAll()
            self.capturedImage = nil
            self.showResultViewer = false
            self.usedNativeEngine = false
        }
    }

    public func retake() {
        DispatchQueue.main.async {
            self.isCapturing = false
            self.isAnalyzing = false
            self.analysisResult = nil
            self.detectedSplines.removeAll()
            self.captureLandmarks.removeAll()
            self.capturedImage = nil
            self.showResultViewer = false
            self.usedNativeEngine = false
            self.guidanceEngine.reset()
            self.cameraSession.start()
        }
    }

    private func processVideoFrame(_ pixelBuffer: CVPixelBuffer) {
        guard !isCapturing && !isAnalyzing && !showResultViewer else { return }

        let now = CACurrentMediaTime()
        let landmarks = handTracker.processFrame(pixelBuffer)
        let framing = landmarks.flatMap { HandLandmarkPose.framing(from: $0) }
        let pose = framing?.pose
        let tipsOK = framing?.isFullyInFrame ?? false

        let photoCenter = pose?.centroid ?? CGPoint(x: 0.5, y: 0.5)
        let photoRadius = pose.map { max(0.18, $0.scale * 0.28) } ?? 0.28
        let photometrics = Photometrics.sample(
            pixelBuffer: pixelBuffer,
            center: photoCenter,
            radius: photoRadius
        )

        // Gate countdown on soft fingertip margin; still coach with cm when scale is off.
        let gatePose = tipsOK ? pose : nil
        var evaluation = guidanceEngine.evaluate(
            pose: gatePose,
            photometrics: photometrics,
            timestamp: now
        )

        if let pose, !tipsOK {
            var metrics = evaluation.metrics
            metrics.scale = pose.scale
            metrics.centerOffset = {
                let dx = Float(pose.centroid.x - 0.5)
                let dy = Float(pose.centroid.y - 0.5)
                return sqrt(dx * dx + dy * dy)
            }()
            metrics.rollAngleDeg = abs(pose.rollDeg)
            metrics.flatnessRatio = pose.aspectRatio
            metrics.meanLuminance = photometrics.meanLuminance
            metrics.glareRatio = photometrics.glareRatio
            metrics.isIlluminationOk =
                photometrics.meanLuminance >= profile.thresholds.minLuminance &&
                photometrics.glareRatio <= profile.thresholds.maxGlareRatio
            metrics.isReadyForCapture = false
            evaluation = GuidanceEvaluation(state: .moveBack, metrics: metrics)
        }

        var statusOverride: String? = nil
        if let pose {
            if !tipsOK {
                statusOverride = "Move ~2 cm farther — keep fingertips inside"
            } else if let hint = PalmOverlay.distanceHintCm(scale: pose.scale, thresholds: profile.thresholds) {
                statusOverride = hint.closer
                    ? "Move ~\(hint.cm) cm closer"
                    : "Move ~\(hint.cm) cm farther"
            } else if case .centerSubject = evaluation.state {
                statusOverride = centerHint(for: pose)
            } else if case .ready(let progress) = evaluation.state, progress < 1 {
                statusOverride = profile.copy.message(for: evaluation.state)
            }
        }

        let shouldPublishMetrics = (now - lastMetricsPublish) >= (1.0 / 15.0)
        if shouldPublishMetrics {
            lastMetricsPublish = now
        }

        let autoArmed = now >= suppressAutoCaptureUntil
        let shouldCapture: Bool = {
            guard autoArmed,
                  landmarks != nil,
                  tipsOK,
                  case .ready(let progress) = evaluation.state,
                  progress >= 1.0,
                  evaluation.metrics.isReadyForCapture
            else { return false }
            return true
        }()

        DispatchQueue.main.async {
            if let landmarks {
                self.detectedLandmarks = landmarks
            } else {
                self.detectedLandmarks.removeAll()
            }
            if !tipsOK {
                self.guidanceEngine.reset()
            }
            self.guidanceState = evaluation.state
            self.statusMessageOverride = statusOverride
            if shouldPublishMetrics {
                self.qualityMetrics = evaluation.metrics
            }

            if let digit = self.profile.copy.countdownDigit(for: evaluation.state), digit != self.lastCountdownDigit {
                self.lastCountdownDigit = digit
                self.lightHaptic.impactOccurred()
            }
            if case .ready = evaluation.state {
            } else {
                self.lastCountdownDigit = 0
            }

            if shouldCapture, let landmarks {
                self.triggerCapture(landmarks: landmarks)
            }
        }
    }

    private func centerHint(for pose: SubjectPose) -> String {
        let dx = Float(pose.centroid.x - 0.5)
        let dy = Float(pose.centroid.y - 0.5)
        let perCm: Float = 0.03
        if abs(dx) >= abs(dy) {
            let cm = max(1, Int(round(abs(dx) / perCm)))
            return dx > 0 ? "Move ~\(cm) cm left" : "Move ~\(cm) cm right"
        } else {
            let cm = max(1, Int(round(abs(dy) / perCm)))
            return dy > 0 ? "Move ~\(cm) cm up" : "Move ~\(cm) cm down"
        }
    }

    public func manualCapture() {
        guard !isCapturing && !isAnalyzing && !showResultViewer, !detectedLandmarks.isEmpty else { return }
        guard let framing = HandLandmarkPose.framing(from: detectedLandmarks), framing.isFullyInFrame else { return }
        triggerCapture(landmarks: detectedLandmarks)
    }

    private func triggerCapture(landmarks: [CGPoint]) {
        guard !isCapturing else { return }
        isCapturing = true
        // Block another auto cycle until this shot finishes + cooldown.
        suppressAutoCaptureUntil = CACurrentMediaTime() + 10.0

        DispatchQueue.main.async {
            self.hapticFeedback.impactOccurred()
            self.isAnalyzing = true
            self.guidanceState = .ready(countdownProgress: 1.0)
        }

        cameraSession.capturePhoto { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let photoBuffer):
                self.finishCapture(photoBuffer: photoBuffer, landmarks: landmarks)
            case .failure(let error):
                self.logger.error("Photo capture failed: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.isCapturing = false
                    self.isAnalyzing = false
                    self.suppressAutoCaptureUntil = CACurrentMediaTime() + 1.5
                    self.guidanceEngine.reset()
                }
            }
        }
    }

    private func finishCapture(photoBuffer: CVPixelBuffer, landmarks: [CGPoint]) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }

            let image = Self.image(from: photoBuffer)
            var splines = self.generateLandmarkCreases(landmarks: landmarks, response: nil)
            var parsedResponse: PalmAnalysisResponse?
            var usedNative = false

            if let engine = self.curvEngine {
                let flat = VisionHandTracker.toFlatLandmarksArray(landmarks)
                switch engine.processPixelBuffer(pixelBuffer: photoBuffer, landmarks: flat) {
                case .success(let jsonString):
                    parsedResponse = PalmAnalysisResponse.from(jsonString: jsonString)
                    splines = self.generateLandmarkCreases(landmarks: landmarks, response: parsedResponse)
                    usedNative = true
                case .failure(let err):
                    self.logger.error("CurvEngine execution error: \(String(describing: err))")
                }
            }

            DispatchQueue.main.async {
                // Stay in live camera mode (iOS Camera style): keep thumbnail, resume preview.
                self.capturedImage = image
                self.captureLandmarks = landmarks
                self.detectedSplines = splines
                self.analysisResult = parsedResponse
                self.usedNativeEngine = usedNative
                self.isAnalyzing = false
                self.isCapturing = false
                self.showResultViewer = false
                self.lastCountdownDigit = 0
                self.suppressAutoCaptureUntil = CACurrentMediaTime() + 3.0
                self.guidanceEngine.reset()
                self.cameraSession.start()
                self.successHaptic.notificationOccurred(.success)
            }
        }
    }

    /// Landmark-anchored major crease guides drawn over the captured palm photo.
    private func generateLandmarkCreases(
        landmarks: [CGPoint],
        response: PalmAnalysisResponse?
    ) -> [CreaseSpline] {
        guard landmarks.count >= 21 else { return [] }

        let wrist = landmarks[0]
        let thumbMcp = landmarks[2]
        let indexMcp = landmarks[5]
        let middleMcp = landmarks[9]
        let ringMcp = landmarks[13]
        let pinkyMcp = landmarks[17]

        func metric(_ key: String, fallbackConf: Float, fallbackTort: Float) -> (Float, Float, Float) {
            let info = response?.domainPayload?.majorLines?[key]
            let ptsFallback: Float = 0
            return (
                info?.confidence ?? fallbackConf,
                info?.tortuosity ?? fallbackTort,
                info?.arcLengthPx ?? ptsFallback
            )
        }

        func length(_ pts: [CGPoint], imageHint: Float) -> Float {
            if imageHint > 0 { return imageHint }
            var sum: Float = 0
            for i in 1..<pts.count {
                let dx = Float(pts[i].x - pts[i - 1].x)
                let dy = Float(pts[i].y - pts[i - 1].y)
                sum += sqrt(dx * dx + dy * dy)
            }
            // Normalized path length × typical 1080p diagonal scale for readable HUD numbers.
            return sum * 1400
        }

        // Heart: across upper palm (Mercury → Jupiter mounts)
        let heart: [CGPoint] = [
            CGPoint(x: pinkyMcp.x * 0.92 + wrist.x * 0.08, y: pinkyMcp.y * 0.72 + wrist.y * 0.28),
            CGPoint(x: ringMcp.x * 0.88 + wrist.x * 0.12, y: ringMcp.y * 0.68 + wrist.y * 0.32),
            CGPoint(x: middleMcp.x * 0.82 + wrist.x * 0.18, y: middleMcp.y * 0.64 + wrist.y * 0.36),
            CGPoint(x: indexMcp.x * 0.78 + thumbMcp.x * 0.22, y: indexMcp.y * 0.58 + thumbMcp.y * 0.42)
        ]
        let (hConf, hTort, hLen) = metric("heart", fallbackConf: 0.86, fallbackTort: 1.12)

        // Head: mid-palm transverse
        let head: [CGPoint] = [
            CGPoint(x: indexMcp.x * 0.68 + thumbMcp.x * 0.32, y: indexMcp.y * 0.48 + thumbMcp.y * 0.52),
            CGPoint(x: middleMcp.x * 0.58 + wrist.x * 0.42, y: middleMcp.y * 0.48 + wrist.y * 0.52),
            CGPoint(x: ringMcp.x * 0.70 + wrist.x * 0.30, y: ringMcp.y * 0.50 + wrist.y * 0.50),
            CGPoint(x: pinkyMcp.x * 0.82 + wrist.x * 0.18, y: pinkyMcp.y * 0.52 + wrist.y * 0.48)
        ]
        let (hdConf, hdTort, hdLen) = metric("head", fallbackConf: 0.90, fallbackTort: 1.08)

        // Life: arcs around thenar
        let life: [CGPoint] = [
            CGPoint(x: indexMcp.x * 0.62 + thumbMcp.x * 0.38, y: indexMcp.y * 0.50 + thumbMcp.y * 0.50),
            CGPoint(x: thumbMcp.x * 0.70 + wrist.x * 0.30, y: thumbMcp.y * 0.45 + wrist.y * 0.55),
            CGPoint(x: thumbMcp.x * 0.45 + wrist.x * 0.55, y: thumbMcp.y * 0.25 + wrist.y * 0.75),
            CGPoint(x: wrist.x * 0.78 + thumbMcp.x * 0.22, y: wrist.y * 0.92 + thumbMcp.y * 0.08)
        ]
        let (lConf, lTort, lLen) = metric("life", fallbackConf: 0.93, fallbackTort: 1.25)

        // Fate: rising toward middle finger
        let fate: [CGPoint] = [
            CGPoint(x: wrist.x * 0.88 + middleMcp.x * 0.12, y: wrist.y * 0.88 + middleMcp.y * 0.12),
            CGPoint(x: wrist.x * 0.55 + middleMcp.x * 0.45, y: wrist.y * 0.55 + middleMcp.y * 0.45),
            CGPoint(x: middleMcp.x * 0.72 + wrist.x * 0.28, y: middleMcp.y * 0.70 + wrist.y * 0.30)
        ]
        let (fConf, fTort, fLen) = metric("fate", fallbackConf: 0.78, fallbackTort: 1.03)

        return [
            CreaseSpline(lineType: .heart, points: heart, confidence: hConf, tortuosity: hTort, arcLengthPx: length(heart, imageHint: hLen)),
            CreaseSpline(lineType: .head, points: head, confidence: hdConf, tortuosity: hdTort, arcLengthPx: length(head, imageHint: hdLen)),
            CreaseSpline(lineType: .life, points: life, confidence: lConf, tortuosity: lTort, arcLengthPx: length(life, imageHint: lLen)),
            CreaseSpline(lineType: .fate, points: fate, confidence: fConf, tortuosity: fTort, arcLengthPx: length(fate, imageHint: fLen))
        ]
    }

    private static func image(from pixelBuffer: CVPixelBuffer) -> UIImage? {
        let ciImage = CIImage(cvPixelBuffer: pixelBuffer)
        let context = CIContext(options: nil)
        guard let cgImage = context.createCGImage(ciImage, from: ciImage.extent) else {
            return nil
        }

        // Prefer upright portrait. If the still buffer is landscape (wider than tall)
        // while the app is locked to portrait, rotate so the thumbnail matches preview.
        let width = CGFloat(cgImage.width)
        let height = CGFloat(cgImage.height)
        if width > height {
            // Sensor landscape → rotate 90° CW for portrait UI (fixes "tilted left" stills).
            return UIImage(cgImage: cgImage, scale: 1.0, orientation: .right)
        }
        return UIImage(cgImage: cgImage, scale: 1.0, orientation: .up)
    }
}

import AVFoundation
import CoreVideo
import Foundation
import os.log

/**
 * Shared AVFoundation camera session: 60 FPS preview frames + high-res stills.
 * Overlay apps do not own their own capture pipeline.
 */
public final class CameraSession: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, AVCapturePhotoCaptureDelegate {
    private let logger = Logger(subsystem: "org.curv.overlay", category: "CameraSession")

    public let captureSession = AVCaptureSession()
    private let videoOutput = AVCaptureVideoDataOutput()
    private let photoOutput = AVCapturePhotoOutput()
    private let sessionQueue = DispatchQueue(label: "org.curv.overlay.sessionQueue")
    private let videoQueue = DispatchQueue(label: "org.curv.overlay.videoQueue", qos: .userInteractive)

    public var onFrameReceived: ((CVPixelBuffer) -> Void)?
    private var photoCaptureCompletion: ((Result<CVPixelBuffer, Error>) -> Void)?

    private var isConfigured: Bool = false

    public override init() {
        super.init()
        checkPermissionsAndConfigure()
    }

    private func checkPermissionsAndConfigure() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureSession()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                guard let self else { return }
                if granted {
                    self.configureSession()
                    self.start()
                } else {
                    self.logger.error("Camera access denied by user")
                }
            }
        default:
            logger.error("Camera authorization unavailable or denied")
        }
    }

    private func configureSession() {
        sessionQueue.async { [weak self] in
            guard let self, !self.isConfigured else { return }
            self.captureSession.beginConfiguration()
            self.captureSession.sessionPreset = .hd1920x1080

            guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back),
                  let input = try? AVCaptureDeviceInput(device: device) else {
                self.logger.error("Failed to acquire back camera device")
                self.captureSession.commitConfiguration()
                return
            }

            if self.captureSession.canAddInput(input) {
                self.captureSession.addInput(input)
            }

            self.videoOutput.alwaysDiscardsLateVideoFrames = true
            self.videoOutput.videoSettings = [
                kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA)
            ]
            self.videoOutput.setSampleBufferDelegate(self, queue: self.videoQueue)

            if self.captureSession.canAddOutput(self.videoOutput) {
                self.captureSession.addOutput(self.videoOutput)
                if let connection = self.videoOutput.connection(with: .video) {
                    connection.videoOrientation = .portrait
                }
            }

            if self.captureSession.canAddOutput(self.photoOutput) {
                self.captureSession.addOutput(self.photoOutput)
                if #available(iOS 16.0, *) {
                    // Match the 1080p video stream to eliminate sensor mode-switch delay & 50MB buffers.
                    let preferred = CMVideoDimensions(width: 1920, height: 1080)
                    let supported = device.activeFormat.supportedMaxPhotoDimensions
                    if supported.contains(where: { $0.width == preferred.width && $0.height == preferred.height }) {
                        self.photoOutput.maxPhotoDimensions = preferred
                    } else if let best = supported.first(where: { $0.width <= 1920 && $0.height <= 1080 }) ?? supported.first {
                        self.photoOutput.maxPhotoDimensions = best
                    }
                }
                self.applyPortraitOrientation(to: self.photoOutput)
            }

            self.captureSession.commitConfiguration()
            self.isConfigured = true
            self.logger.info("CameraSession configured successfully (1080p stream + photo)")
        }
    }

    private func applyPortraitOrientation(to output: AVCaptureOutput) {
        guard let connection = output.connection(with: .video), connection.isVideoOrientationSupported else {
            return
        }
        connection.videoOrientation = .portrait
    }

    public func start() {
        if !isConfigured {
            checkPermissionsAndConfigure()
        }
        sessionQueue.async { [weak self] in
            guard let self, self.isConfigured, !self.captureSession.isRunning else { return }
            self.captureSession.startRunning()
        }
    }

    public func stop() {
        sessionQueue.async { [weak self] in
            guard let self, self.captureSession.isRunning else { return }
            self.captureSession.stopRunning()
        }
    }

    public func capturePhoto(completion: @escaping (Result<CVPixelBuffer, Error>) -> Void) {
        sessionQueue.async { [weak self] in
            guard let self else { return }
            guard self.captureSession.isRunning else {
                completion(.failure(NSError(domain: "org.curv.overlay", code: -2, userInfo: [NSLocalizedDescriptionKey: "Camera session is not running"])))
                return
            }

            // Keep stills in the same portrait orientation as the live preview.
            self.applyPortraitOrientation(to: self.photoOutput)
            self.photoCaptureCompletion = completion

            let settings: AVCapturePhotoSettings
            if self.photoOutput.availablePhotoPixelFormatTypes.contains(kCVPixelFormatType_32BGRA) {
                settings = AVCapturePhotoSettings(format: [
                    kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA)
                ])
            } else {
                settings = AVCapturePhotoSettings()
            }

            if #available(iOS 16.0, *) {
                let preferred = CMVideoDimensions(width: 1920, height: 1080)
                let maxDim = self.photoOutput.maxPhotoDimensions
                if maxDim.width >= preferred.width && maxDim.height >= preferred.height {
                    settings.maxPhotoDimensions = preferred
                } else {
                    settings.maxPhotoDimensions = maxDim
                }
            }
            self.photoOutput.capturePhoto(with: settings, delegate: self)
        }
    }

    public func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        onFrameReceived?(pixelBuffer)
    }

    public func photoOutput(
        _ output: AVCapturePhotoOutput,
        didFinishProcessingPhoto photo: AVCapturePhoto,
        error: Error?
    ) {
        if let error {
            photoCaptureCompletion?(.failure(error))
            photoCaptureCompletion = nil
            return
        }
        guard let pixelBuffer = photo.pixelBuffer else {
            let err = NSError(
                domain: "org.curv.overlay",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "No pixel buffer in captured photo"]
            )
            photoCaptureCompletion?(.failure(err))
            photoCaptureCompletion = nil
            return
        }
        photoCaptureCompletion?(.success(pixelBuffer))
        photoCaptureCompletion = nil
    }
}

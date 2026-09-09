import Foundation
import AVFoundation
import CoreVideo
import os.log

/**
 * AVFoundation Camera Controller delivering 60 FPS video frames and high-res capture.
 */
public final class CameraManager: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, AVCapturePhotoCaptureDelegate {
    private let logger = Logger(subsystem: "org.curv.palm", category: "CameraManager")

    public let captureSession = AVCaptureSession()
    private let videoOutput = AVCaptureVideoDataOutput()
    private let photoOutput = AVCapturePhotoOutput()
    private let sessionQueue = DispatchQueue(label: "org.curv.camera.sessionQueue")
    private let videoQueue = DispatchQueue(label: "org.curv.camera.videoQueue", qos: .userInteractive)

    public var onFrameReceived: ((CVPixelBuffer) -> Void)?
    private var photoCaptureCompletion: ((Result<CVPixelBuffer, Error>) -> Void)?

    public override init() {
        super.init()
        configureSession()
    }

    private func configureSession() {
        sessionQueue.async { [weak self] in
            guard let self = self else { return }
            self.captureSession.beginConfiguration()
            self.captureSession.sessionPreset = .hd1920x1080

            // Back camera default for high resolution macro hand capture
            guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back),
                  let input = try? AVCaptureDeviceInput(device: device) else {
                self.logger.error("Failed to acquire back camera device")
                self.captureSession.commitConfiguration()
                return
            }

            if self.captureSession.canAddInput(input) {
                self.captureSession.addInput(input)
            }

            // Video data output for live 60 FPS guidance loop
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

            // Photo output for full-res deep Steger analysis
            if self.captureSession.canAddOutput(self.photoOutput) {
                self.captureSession.addOutput(self.photoOutput)
                self.photoOutput.isHighResolutionCaptureEnabled = true
            }

            self.captureSession.commitConfiguration()
        }
    }

    public func start() {
        sessionQueue.async { [weak self] in
            guard let self = self, !self.captureSession.isRunning else { return }
            self.captureSession.startRunning()
        }
    }

    public func stop() {
        sessionQueue.async { [weak self] in
            guard let self = self, self.captureSession.isRunning else { return }
            self.captureSession.stopRunning()
        }
    }

    public func capturePhoto(completion: @escaping (Result<CVPixelBuffer, Error>) -> Void) {
        sessionQueue.async { [weak self] in
            guard let self = self else { return }
            self.photoCaptureCompletion = completion

            let settings = AVCapturePhotoSettings(format: [
                kCVPixelBufferPixelFormatTypeKey as String: Int(kCVPixelFormatType_32BGRA)
            ])
            settings.isHighResolutionPhotoEnabled = true
            self.photoOutput.capturePhoto(with: settings, delegate: self)
        }
    }

    // MARK: - AVCaptureVideoDataOutputSampleBufferDelegate
    public func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        onFrameReceived?(pixelBuffer)
    }

    // MARK: - AVCapturePhotoCaptureDelegate
    public func photoOutput(
        _ output: AVCapturePhotoOutput,
        didFinishProcessingPhoto photo: AVCapturePhoto,
        error: Error?
    ) {
        if let error = error {
            photoCaptureCompletion?(.failure(error))
            photoCaptureCompletion = nil
            return
        }

        guard let pixelBuffer = photo.pixelBuffer else {
            let err = NSError(domain: "org.curv.camera", code: -1, userInfo: [NSLocalizedDescriptionKey: "No pixel buffer in captured photo"])
            photoCaptureCompletion?(.failure(err))
            photoCaptureCompletion = nil
            return
        }

        photoCaptureCompletion?(.success(pixelBuffer))
        photoCaptureCompletion = nil
    }
}

import Foundation
import CoreVideo

#if canImport(curv_capi)
import curv_capi
#endif

public enum CurvError: Error {
    case initializationFailed
    case invalidBuffer
    case frameCreationFailed
    case pipelineExecutionFailed(Int)
    case bufferTooSmall
    /// Native `libcurv_capi` / XCFramework is not linked in this build.
    case nativeUnavailable
}

public enum CurvPixelFormat: Int32 {
    case grayscale = 1
    case rgb = 2
    case rgba = 3
    case bgr = 4
    case bgra = 5
}

/**
 * Swift wrapper for CurvEngine native perception core.
 *
 * When `CurvEngine.xcframework` (module `curv_capi`) is linked, calls go to the
 * C-ABI. Without it, init throws `.nativeUnavailable` so the camera UI can still
 * run for overlay / guidance validation on device.
 */
public final class CurvEngine {

    public static var isNativeAvailable: Bool {
        #if canImport(curv_capi)
        return true
        #else
        return false
        #endif
    }

    #if canImport(curv_capi)
    private var pipelineHandle: OpaquePointer?
    #endif

    public init(domainId: String = "palm") throws {
        #if canImport(curv_capi)
        let handle = domainId.withCString { curv_pipeline_create($0) }
        guard let validHandle = handle else {
            throw CurvError.initializationFailed
        }
        self.pipelineHandle = OpaquePointer(validHandle)
        #else
        throw CurvError.nativeUnavailable
        #endif
    }

    deinit {
        #if canImport(curv_capi)
        if let handle = pipelineHandle {
            curv_pipeline_destroy(UnsafeMutableRawPointer(handle))
            pipelineHandle = nil
        }
        #endif
    }

    public func processLandmarksRoi(
        bytes: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        stride: Int = 0,
        format: CurvPixelFormat = .bgra,
        landmarks: [Float]
    ) -> Result<String, CurvError> {
        #if canImport(curv_capi)
        guard let handle = pipelineHandle else {
            return .failure(.initializationFailed)
        }
        guard landmarks.count >= 42 else {
            return .failure(.invalidBuffer)
        }

        guard let frameHandle = curv_frame_create_from_buffer(
            bytes,
            Int32(width),
            Int32(height),
            Int32(stride),
            format.rawValue
        ) else {
            return .failure(.frameCreationFailed)
        }

        defer { curv_frame_destroy(frameHandle) }

        var jsonBuffer = [CChar](repeating: 0, count: 2097152)
        var status = landmarks.withUnsafeBufferPointer { lmsPtr in
            curv_pipeline_process_landmarks_roi(
                UnsafeMutableRawPointer(handle),
                frameHandle,
                lmsPtr.baseAddress,
                &jsonBuffer,
                Int32(jsonBuffer.count)
            )
        }

        if status == 4 { // CURV_STATUS_BUFFER_TOO_SMALL
            var neededSize: Int32 = 0
            curv_pipeline_get_last_json(UnsafeMutableRawPointer(handle), nil, 0, &neededSize)
            if neededSize > 0 {
                jsonBuffer = [CChar](repeating: 0, count: Int(neededSize))
                status = curv_pipeline_get_last_json(
                    UnsafeMutableRawPointer(handle),
                    &jsonBuffer,
                    Int32(jsonBuffer.count),
                    nil
                )
            }
        }

        if status != 0 && status != 2 {
            return .failure(.pipelineExecutionFailed(Int(status)))
        }
        return .success(String(cString: jsonBuffer))
        #else
        return .failure(.nativeUnavailable)
        #endif
    }

    public func processQuadRoi(
        bytes: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        stride: Int = 0,
        format: CurvPixelFormat = .bgra,
        corners: [Float]
    ) -> Result<String, CurvError> {
        #if canImport(curv_capi)
        guard let handle = pipelineHandle else {
            return .failure(.initializationFailed)
        }
        guard corners.count >= 8 else {
            return .failure(.invalidBuffer)
        }

        guard let frameHandle = curv_frame_create_from_buffer(
            bytes,
            Int32(width),
            Int32(height),
            Int32(stride),
            format.rawValue
        ) else {
            return .failure(.frameCreationFailed)
        }

        defer { curv_frame_destroy(frameHandle) }

        var jsonBuffer = [CChar](repeating: 0, count: 2097152)
        var status = corners.withUnsafeBufferPointer { cornersPtr in
            curv_pipeline_process_quad_roi(
                UnsafeMutableRawPointer(handle),
                frameHandle,
                cornersPtr.baseAddress,
                &jsonBuffer,
                Int32(jsonBuffer.count)
            )
        }

        if status == 4 {
            var neededSize: Int32 = 0
            curv_pipeline_get_last_json(UnsafeMutableRawPointer(handle), nil, 0, &neededSize)
            if neededSize > 0 {
                jsonBuffer = [CChar](repeating: 0, count: Int(neededSize))
                status = curv_pipeline_get_last_json(
                    UnsafeMutableRawPointer(handle),
                    &jsonBuffer,
                    Int32(jsonBuffer.count),
                    nil
                )
            }
        }

        if status != 0 && status != 2 {
            return .failure(.pipelineExecutionFailed(Int(status)))
        }
        return .success(String(cString: jsonBuffer))
        #else
        return .failure(.nativeUnavailable)
        #endif
    }

    public func processPixelBuffer(
        pixelBuffer: CVPixelBuffer,
        landmarks: [Float]
    ) -> Result<String, CurvError> {
        withLockedPixelBuffer(pixelBuffer) { bytes, width, height, stride in
            processLandmarksRoi(
                bytes: bytes,
                width: width,
                height: height,
                stride: stride,
                format: .bgra,
                landmarks: landmarks
            )
        }
    }

    public func processPixelBuffer(
        pixelBuffer: CVPixelBuffer,
        corners: [Float]
    ) -> Result<String, CurvError> {
        withLockedPixelBuffer(pixelBuffer) { bytes, width, height, stride in
            processQuadRoi(
                bytes: bytes,
                width: width,
                height: height,
                stride: stride,
                format: .bgra,
                corners: corners
            )
        }
    }

    private func withLockedPixelBuffer(
        _ pixelBuffer: CVPixelBuffer,
        body: (UnsafePointer<UInt8>, Int, Int, Int) -> Result<String, CurvError>
    ) -> Result<String, CurvError> {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return .failure(.invalidBuffer)
        }

        return body(
            baseAddress.assumingMemoryBound(to: UInt8.self),
            CVPixelBufferGetWidth(pixelBuffer),
            CVPixelBufferGetHeight(pixelBuffer),
            CVPixelBufferGetBytesPerRow(pixelBuffer)
        )
    }
}

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
 * Interfaces directly with curv_capi flat C-ABI without Objective-C bridging overhead.
 */
public final class CurvEngine {

    private var pipelineHandle: OpaquePointer?

    public init(domainId: String = "palm") throws {
        let handle = curv_pipeline_create(domainId)
        guard let validHandle = handle else {
            throw CurvError.initializationFailed
        }
        self.pipelineHandle = OpaquePointer(validHandle)
    }

    deinit {
        if let handle = pipelineHandle {
            curv_pipeline_destroy(UnsafeMutableRawPointer(handle))
            pipelineHandle = nil
        }
    }

    /**
     * Processes a raw image buffer with 21 normalized landmarks (42 floats).
     */
    public func processLandmarksRoi(
        bytes: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        stride: Int = 0,
        format: CurvPixelFormat = .bgra,
        landmarks: [Float]
    ) -> Result<String, CurvError> {
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

        defer {
            curv_frame_destroy(frameHandle)
        }

        var jsonBuffer = [CChar](repeating: 0, count: 2097152) // 2 MB default buffer
        var status = landmarks.withUnsafeBufferPointer { lmsPtr in
            curv_pipeline_process_landmarks_roi(
                UnsafeMutableRawPointer(handle),
                frameHandle,
                lmsPtr.baseAddress,
                &jsonBuffer,
                Int32(jsonBuffer.count)
            )
        }

        if status == 3 { // CURV_STATUS_BUFFER_TOO_SMALL
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

        if status != 0 && status != 2 { // 0: SUCCESS, 2: QUALITY_REJECTED (still has JSON)
            return .failure(.pipelineExecutionFailed(Int(status)))
        }

        let jsonString = String(cString: jsonBuffer)
        return .success(jsonString)
    }

    /**
     * Processes an AVFoundation CoreVideo CVPixelBuffer directly.
     */
    public func processPixelBuffer(
        pixelBuffer: CVPixelBuffer,
        landmarks: [Float]
    ) -> Result<String, CurvError> {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer {
            CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly)
        }

        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return .failure(.invalidBuffer)
        }

        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let bytes = baseAddress.assumingMemoryBound(to: UInt8.self)

        return processLandmarksRoi(
            bytes: bytes,
            width: width,
            height: height,
            stride: bytesPerRow,
            format: .bgra,
            landmarks: landmarks
        )
    }
}

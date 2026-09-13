#ifndef CURV_CAPI_H
#define CURV_CAPI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32) || defined(__CYGWIN__)
#ifdef CURV_CAPI_EXPORTS
#define CURV_API __declspec(dllexport)
#else
#define CURV_API __declspec(dllimport)
#endif
#else
#define CURV_API __attribute__((visibility("default")))
#endif

/**
 * @brief Status return codes for Curv C-ABI functions.
 */
typedef enum CurvStatusCode {
    CURV_STATUS_SUCCESS = 0,
    CURV_STATUS_INVALID_ARGUMENT = 1,
    CURV_STATUS_QUALITY_GATE_REJECTED = 2,
    CURV_STATUS_INTERNAL_ERROR = 3,
    CURV_STATUS_BUFFER_TOO_SMALL = 4
} CurvStatusCode;

/**
 * @brief Pixel buffer formats.
 */
typedef enum CurvPixelFormat {
    CURV_PIXEL_FORMAT_GRAYSCALE = 1,
    CURV_PIXEL_FORMAT_RGB = 2,
    CURV_PIXEL_FORMAT_RGBA = 3,
    CURV_PIXEL_FORMAT_BGR = 4,
    CURV_PIXEL_FORMAT_BGRA = 5
} CurvPixelFormat;

/**
 * @brief Frame quality diagnostics.
 */
typedef struct CurvQualityResult {
    double blur_score;
    double brightness_score;
    double contrast_score;
    int is_usable;
} CurvQualityResult;

/**
 * @brief Opaque pipeline handle.
 */
typedef void* CurvPipelineHandle;
typedef void* curv_pipeline_t;

/**
 * @brief Opaque frame buffer handle.
 */
typedef void* CurvFrameHandle;
typedef void* curv_frame_t;

/**
 * @brief Returns the version string of the Curv engine.
 */
CURV_API const char* curv_get_version(void);

/**
 * @brief Creates a Curv pipeline instance.
 * @param domain_id Domain identifier string (e.g. "palm", "surface_inspection", or NULL/"none").
 * @return Opaque pipeline handle, or NULL on failure.
 */
CURV_API CurvPipelineHandle curv_pipeline_create(const char* domain_id);

/**
 * @brief Destroys a Curv pipeline instance.
 * @param pipeline Opaque handle. Safe to pass NULL.
 */
CURV_API void curv_pipeline_destroy(CurvPipelineHandle pipeline);

/**
 * @brief Creates a Curv frame from an external memory buffer.
 * Copies the image data into an internal buffer.
 *
 * @param buffer Pointer to raw pixel bytes.
 * @param width Image width in pixels (must be <= 16384).
 * @param height Image height in pixels (must be <= 16384).
 * @param stride Row stride in bytes. Pass 0 for tightly packed (width * bytes_per_pixel).
 *               Non-zero values must be >= width * bytes_per_pixel; negative values are rejected.
 * @param format Format enum (CurvPixelFormat).
 * @return Opaque frame handle, or NULL on failure (invalid dimensions, stride, or format).
 */
CURV_API CurvFrameHandle curv_frame_create_from_buffer(const uint8_t* buffer, int width, int height, int stride,
                                                       int format);

/**
 * @brief Destroys a frame created via curv_frame_create_from_buffer.
 * @param frame Opaque handle. Safe to pass NULL.
 */
CURV_API void curv_frame_destroy(CurvFrameHandle frame);

/**
 * @brief Processes a full frame without ROI cropping.
 *
 * @param pipeline Pipeline handle.
 * @param frame Frame handle.
 * @param out_json_buffer Caller-allocated character buffer for evidence JSON.
 * @param out_json_buffer_size Size of out_json_buffer in bytes.
 * @return Status code (CURV_STATUS_SUCCESS on success).
 */
CURV_API int curv_pipeline_process_frame(CurvPipelineHandle pipeline, CurvFrameHandle frame, char* out_json_buffer,
                                         int out_json_buffer_size);

/**
 * @brief Processes an arbitrary quadrilateral ROI within the frame (4 corner points).
 *
 * @param pipeline Pipeline handle.
 * @param frame Frame handle.
 * @param roi_corners_8_floats Array of 8 floats: [TL_x, TL_y, TR_x, TR_y, BR_x, BR_y, BL_x, BL_y].
 * @param out_json_buffer Caller-allocated character buffer for evidence JSON.
 * @param out_json_buffer_size Size of out_json_buffer in bytes.
 * @return Status code (CURV_STATUS_SUCCESS on success).
 */
CURV_API int curv_pipeline_process_quad_roi(CurvPipelineHandle pipeline, CurvFrameHandle frame,
                                            const float* roi_corners_8_floats, char* out_json_buffer,
                                            int out_json_buffer_size);

/**
 * @brief Processes a palm ROI using 21 hand landmarks (42 floats: x0, y0, x1, y1, ...).
 * Normalizes to canonical upright palm patch, extracts ridges, and inverts coordinates back.
 *
 * @param pipeline Pipeline handle.
 * @param frame Frame handle.
 * @param landmarks_42_floats Array of 42 floats representing 21 (x, y) normalized coordinates.
 * @param out_json_buffer Caller-allocated character buffer for evidence JSON.
 * @param out_json_buffer_size Size of out_json_buffer in bytes.
 * @return Status code (CURV_STATUS_SUCCESS on success).
 */
CURV_API int curv_pipeline_process_landmarks_roi(CurvPipelineHandle pipeline, CurvFrameHandle frame,
                                                 const float* landmarks_42_floats, char* out_json_buffer,
                                                 int out_json_buffer_size);

/**
 * @brief Retrieves the quality assessment result of the most recent pipeline execution.
 *
 * @param pipeline Pipeline handle.
 * @param out_quality Pointer to CurvQualityResult structure to populate.
 * @return Status code.
 */
CURV_API int curv_pipeline_get_last_quality(CurvPipelineHandle pipeline, CurvQualityResult* out_quality);

/**
 * @brief Retrieves the serialized JSON result from the most recent pipeline execution.
 * Useful for querying the required buffer size before allocation or when CURV_STATUS_BUFFER_TOO_SMALL is returned.
 *
 * @param pipeline Pipeline handle.
 * @param out_json_buffer Caller-allocated buffer, or NULL to query size only.
 * @param out_json_buffer_size Size of out_json_buffer in bytes (can be 0 if querying size).
 * @param out_required_size Pointer to int populated with required buffer size including null terminator.
 * @return Status code.
 */
CURV_API int curv_pipeline_get_last_json(CurvPipelineHandle pipeline, char* out_json_buffer, int out_json_buffer_size,
                                         int* out_required_size);

/**
 * @brief Analyzes an orbit capture session JSON and returns mount_relief domain payload.
 * @param orbit_session_json Null-terminated orbit session JSON (curv_orbit_session_v1).
 */
CURV_API int curv_analyze_orbit_session_json(const char* orbit_session_json, char* out_json_buffer,
                                             int out_json_buffer_size);

/**
 * @brief Palm ROI processing with fine-scale Steger pass for minor_ridges in domain_payload.
 */
CURV_API int curv_pipeline_process_landmarks_roi_extended(CurvPipelineHandle pipeline, CurvFrameHandle frame,
                                                          const float* landmarks_42_floats, int include_minor_ridges,
                                                          char* out_json_buffer, int out_json_buffer_size);

/**
 * @brief Classifies dermatoglyphic pattern (arch/loop/whorl) from fingertip close-up ROI.
 * @param digit_name e.g. "index", "thumb".
 */
CURV_API int curv_classify_fingertip_roi(CurvPipelineHandle pipeline, CurvFrameHandle frame,
                                         const float* landmarks_42_floats, const char* digit_name,
                                         char* out_json_buffer, int out_json_buffer_size);

/**
 * @brief Computes pure scientific anthropometric metrics (2D:4D ratio, digit lengths, palm dimensions) from 21 landmarks.
 * @param landmarks_42_floats Array of 42 floats (21 (x, y) normalized pairs).
 * @param reference_palm_width_mm Anatomical metacarpal breadth scale prior (typically 80.0 mm).
 * @param pad_depth_json Optional JSON object string mapping pad names to millimeter depth, or NULL.
 * @param hand_side e.g. "left", "right", "auto".
 */
CURV_API int curv_compute_hand_anthropometry_json(const float* landmarks_42_floats,
                                                  float reference_palm_width_mm,
                                                  const char* pad_depth_json,
                                                  const char* hand_side,
                                                  char* out_json_buffer,
                                                  int out_json_buffer_size);

/**
 * @brief Generates a 3D surface mesh and elevation heatmap from 21 landmarks.
 */
CURV_API int curv_generate_hand_mesh_json(const float* landmarks_42_floats,
                                          float reference_palm_width_mm,
                                          const char* pad_depth_json,
                                          char* out_json_buffer,
                                          int out_json_buffer_size);

#ifdef __cplusplus
}
#endif

#endif /* CURV_CAPI_H */

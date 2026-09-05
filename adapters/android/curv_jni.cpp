#include "curv/curv_capi.h"
#include <jni.h>
#include <string>
#include <vector>

namespace {

// Mirrors the channel mapping enforced inside curv_frame_create_from_buffer so
// the zero-copy path can validate the caller's direct buffer up front and
// report a precise error instead of a generic frame_creation_failed.
jint channel_count_for_format(jint format) {
    switch (format) {
    case CURV_PIXEL_FORMAT_GRAYSCALE:
        return 1;
    case CURV_PIXEL_FORMAT_RGB:
    case CURV_PIXEL_FORMAT_BGR:
        return 3;
    case CURV_PIXEL_FORMAT_RGBA:
    case CURV_PIXEL_FORMAT_BGRA:
        return 4;
    default:
        return 0;
    }
}

} // namespace

extern "C" {

JNIEXPORT jlong JNICALL Java_org_curv_CurvPipeline_nativeCreate(JNIEnv* env, jobject /*thiz*/, jstring domain_id) {
    const char* c_domain = domain_id ? env->GetStringUTFChars(domain_id, nullptr) : nullptr;
    CurvPipelineHandle handle = curv_pipeline_create(c_domain);
    if (c_domain) {
        env->ReleaseStringUTFChars(domain_id, c_domain);
    }
    return reinterpret_cast<jlong>(handle);
}

JNIEXPORT void JNICALL Java_org_curv_CurvPipeline_nativeDestroy(JNIEnv* /*env*/, jobject /*thiz*/, jlong handle) {
    auto p_handle = reinterpret_cast<CurvPipelineHandle>(handle);
    curv_pipeline_destroy(p_handle);
}

JNIEXPORT jstring JNICALL Java_org_curv_CurvPipeline_nativeProcessLandmarksRoi(JNIEnv* env, jobject /*thiz*/,
                                                                               jlong handle, jobject byte_buffer,
                                                                               jint width, jint height, jint stride,
                                                                               jint format,
                                                                               jfloatArray landmarks_array) {
    auto p_handle = reinterpret_cast<CurvPipelineHandle>(handle);
    if (!p_handle || !byte_buffer) {
        return env->NewStringUTF("{\"error\": \"invalid_argument\"}");
    }

    // Zero-copy direct buffer access from CameraX ImageProxy
    auto* buffer_ptr = static_cast<const uint8_t*>(env->GetDirectBufferAddress(byte_buffer));
    if (!buffer_ptr) {
        return env->NewStringUTF("{\"error\": \"non_direct_buffer\"}");
    }

    // Validate geometry and buffer capacity before handing the raw pointer to
    // the engine: a short direct buffer would otherwise be read out of bounds.
    const jint channels = channel_count_for_format(format);
    if (channels <= 0 || width <= 0 || height <= 0 || stride < 0) {
        return env->NewStringUTF("{\"error\": \"invalid_frame_geometry\"}");
    }

    const jlong min_row_bytes = static_cast<jlong>(width) * static_cast<jlong>(channels);
    const jlong row_bytes = (stride > 0) ? static_cast<jlong>(stride) : min_row_bytes;
    if (row_bytes < min_row_bytes) {
        return env->NewStringUTF("{\"error\": \"invalid_frame_geometry\"}");
    }
    const jlong required_bytes = row_bytes * static_cast<jlong>(height);
    const jlong capacity = env->GetDirectBufferCapacity(byte_buffer);
    if (capacity < required_bytes) {
        return env->NewStringUTF("{\"error\": \"buffer_too_small\"}");
    }

    CurvFrameHandle f_handle = curv_frame_create_from_buffer(buffer_ptr, width, height, stride, format);

    if (!f_handle) {
        return env->NewStringUTF("{\"error\": \"frame_creation_failed\"}");
    }

    if (!landmarks_array || env->GetArrayLength(landmarks_array) < 42) {
        curv_frame_destroy(f_handle);
        return env->NewStringUTF("{\"error\": \"invalid_landmarks\"}");
    }

    std::vector<float> lms_data(42, 0.0f);
    env->GetFloatArrayRegion(landmarks_array, 0, 42, lms_data.data());

    std::vector<char> json_buf(2097152, 0); // 2 MB default buffer
    int status = curv_pipeline_process_landmarks_roi(p_handle, f_handle, lms_data.data(), json_buf.data(),
                                                     static_cast<int>(json_buf.size()));

    if (status == CURV_STATUS_BUFFER_TOO_SMALL) {
        int needed_size = 0;
        curv_pipeline_get_last_json(p_handle, nullptr, 0, &needed_size);
        if (needed_size > 0) {
            json_buf.resize(static_cast<size_t>(needed_size), 0);
            status = curv_pipeline_get_last_json(p_handle, json_buf.data(), static_cast<int>(json_buf.size()), nullptr);
        }
    }

    curv_frame_destroy(f_handle);

    if (status != CURV_STATUS_SUCCESS && status != CURV_STATUS_QUALITY_GATE_REJECTED) {
        return env->NewStringUTF("{\"error\": \"pipeline_execution_failed\"}");
    }

    return env->NewStringUTF(json_buf.data());
}

} // extern "C"

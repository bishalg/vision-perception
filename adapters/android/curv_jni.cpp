#include <jni.h>
#include <string>
#include <vector>
#include "curv/curv_capi.h"

extern "C" {

JNIEXPORT jlong JNICALL
Java_org_curv_CurvPipeline_nativeCreate(JNIEnv* env, jobject /*thiz*/, jstring domain_id) {
    const char* c_domain = domain_id ? env->GetStringUTFChars(domain_id, nullptr) : nullptr;
    CurvPipelineHandle handle = curv_pipeline_create(c_domain);
    if (c_domain) {
        env->ReleaseStringUTFChars(domain_id, c_domain);
    }
    return reinterpret_cast<jlong>(handle);
}

JNIEXPORT void JNICALL
Java_org_curv_CurvPipeline_nativeDestroy(JNIEnv* /*env*/, jobject /*thiz*/, jlong handle) {
    auto p_handle = reinterpret_cast<CurvPipelineHandle>(handle);
    curv_pipeline_destroy(p_handle);
}

JNIEXPORT jstring JNICALL
Java_org_curv_CurvPipeline_nativeProcessLandmarksRoi(
    JNIEnv* env,
    jobject /*thiz*/,
    jlong handle,
    jobject byte_buffer,
    jint width,
    jint height,
    jint stride,
    jint format,
    jfloatArray landmarks_array
) {
    auto p_handle = reinterpret_cast<CurvPipelineHandle>(handle);
    if (!p_handle || !byte_buffer) {
        return env->NewStringUTF("{\"error\": \"invalid_argument\"}");
    }

    // Zero-copy direct buffer access from CameraX ImageProxy
    auto* buffer_ptr = static_cast<const uint8_t*>(env->GetDirectBufferAddress(byte_buffer));
    if (!buffer_ptr) {
        return env->NewStringUTF("{\"error\": \"non_direct_buffer\"}");
    }

    CurvFrameHandle f_handle = curv_frame_create_from_buffer(
        buffer_ptr,
        width,
        height,
        stride,
        format
    );

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
    int status = curv_pipeline_process_landmarks_roi(
        p_handle,
        f_handle,
        lms_data.data(),
        json_buf.data(),
        static_cast<int>(json_buf.size())
    );

    if (status == CURV_STATUS_BUFFER_TOO_SMALL) {
        int needed_size = 0;
        curv_pipeline_get_last_json(p_handle, nullptr, 0, &needed_size);
        if (needed_size > 0) {
            json_buf.resize(static_cast<size_t>(needed_size), 0);
            status = curv_pipeline_get_last_json(
                p_handle,
                json_buf.data(),
                static_cast<int>(json_buf.size()),
                nullptr
            );
        }
    }

    curv_frame_destroy(f_handle);

    if (status != CURV_STATUS_SUCCESS && status != CURV_STATUS_QUALITY_GATE_REJECTED) {
        return env->NewStringUTF("{\"error\": \"pipeline_execution_failed\"}");
    }

    return env->NewStringUTF(json_buf.data());
}

} // extern "C"

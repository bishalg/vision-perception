#include "curv/curv_capi.h"

#include <cstring>
#include <string>
#include <memory>
#include <vector>
#include <array>
#include <exception>
#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>
#include <nlohmann/json.hpp>

#include "curv/Version.hpp"
#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/geometry/RoiWarper.hpp"
#include "curv/config/DynamicParameterResolver.hpp"
#include "curv/domain/PalmParameters.hpp"
#include "curv/domains/SurfaceInspectionPack.hpp"
#include "curv/domains/PalmDomainPack.hpp"

namespace {

struct PipelineContext {
    std::shared_ptr<CurvEngine::quality::LaplacianQualityAnalyzer> quality_analyzer;
    std::shared_ptr<CurvEngine::ridge::StegerRidgeExtractor> ridge_extractor;
    std::unique_ptr<CurvEngine::pipeline::SyncPerceptionPipeline> pipeline;
    std::unique_ptr<CurvEngine::IDomainPack> domain_pack;
    CurvEngine::config::DynamicParameterResolver param_resolver;
    CurvQualityResult last_quality{0.0, 0.0, 0.0, 0};
    std::string domain_id;
    std::string last_json;

    PipelineContext(const char* domain) {
        if (domain) {
            domain_id = domain;
        }

        CurvEngine::quality::QualityThresholds q_thresh;
        q_thresh.min_blur_score = 25.0;
        q_thresh.min_brightness = 10.0;
        q_thresh.max_brightness = 245.0;
        q_thresh.min_contrast = 10.0;
        quality_analyzer = std::make_shared<CurvEngine::quality::LaplacianQualityAnalyzer>(q_thresh);

        CurvEngine::ridge::StegerConfig r_cfg;
        r_cfg.sigma = 1.5f;
        r_cfg.low_threshold = 0.5f;
        r_cfg.high_threshold = 1.5f;
        r_cfg.min_segment_length = 5.0f;
        r_cfg.extract_dark_lines = false;
        ridge_extractor = std::make_shared<CurvEngine::ridge::StegerRidgeExtractor>(r_cfg);

        pipeline = std::make_unique<CurvEngine::pipeline::SyncPerceptionPipeline>(quality_analyzer, ridge_extractor);

        if (domain_id == "palm" || domain_id == "org.curv.domain.palm") {
            domain_pack = std::make_unique<CurvEngine::domains::PalmDomainPack>();
        } else if (domain_id == "surface_inspection" || domain_id == "org.curv.domain.surface_inspection") {
            domain_pack = std::make_unique<CurvEngine::domains::SurfaceInspectionPack>();
        }
    }
};

struct FrameContext {
    CurvEngine::Frame frame;

    FrameContext(const cv::Mat& mat, const std::string& src_id, uint64_t fid)
        : frame(mat, src_id, fid) {}
};

int copy_json_to_buffer(const nlohmann::json& js, char* out_buf, int buf_size) {
    if (!out_buf || buf_size <= 0) {
        return CURV_STATUS_INVALID_ARGUMENT;
    }
    out_buf[0] = '\0';

    const std::string serialized = js.dump();
    const size_t needed_len = serialized.length() + 1;

    if (static_cast<size_t>(buf_size) < needed_len) {
        return CURV_STATUS_BUFFER_TOO_SMALL;
    }

    std::memcpy(out_buf, serialized.c_str(), needed_len);
    return CURV_STATUS_SUCCESS;
}

} // anonymous namespace

extern "C" {

const char* curv_get_version(void) {
    static const std::string s_ver = CurvEngine::getVersion();
    return s_ver.c_str();
}

CurvPipelineHandle curv_pipeline_create(const char* domain_id) {
    try {
        auto* ctx = new PipelineContext(domain_id);
        return static_cast<CurvPipelineHandle>(ctx);
    } catch (...) {
        return nullptr;
    }
}

void curv_pipeline_destroy(CurvPipelineHandle pipeline) {
    if (!pipeline) return;
    try {
        auto* ctx = static_cast<PipelineContext*>(pipeline);
        delete ctx;
    } catch (...) {}
}

CurvFrameHandle curv_frame_create_from_buffer(
    const uint8_t* buffer,
    int width,
    int height,
    int stride,
    int format
) {
    if (!buffer || width <= 0 || height <= 0) {
        return nullptr;
    }

    try {
        int cv_type = CV_8UC3;
        int channels = 3;
        bool needs_bgr_conversion = false;
        int conversion_code = 0;

        switch (format) {
            case CURV_PIXEL_FORMAT_GRAYSCALE:
                cv_type = CV_8UC1;
                channels = 1;
                break;
            case CURV_PIXEL_FORMAT_RGB:
                cv_type = CV_8UC3;
                channels = 3;
                needs_bgr_conversion = true;
                conversion_code = cv::COLOR_RGB2BGR;
                break;
            case CURV_PIXEL_FORMAT_RGBA:
                cv_type = CV_8UC4;
                channels = 4;
                needs_bgr_conversion = true;
                conversion_code = cv::COLOR_RGBA2BGR;
                break;
            case CURV_PIXEL_FORMAT_BGR:
                cv_type = CV_8UC3;
                channels = 3;
                break;
            case CURV_PIXEL_FORMAT_BGRA:
                cv_type = CV_8UC4;
                channels = 4;
                needs_bgr_conversion = true;
                conversion_code = cv::COLOR_BGRA2BGR;
                break;
            default:
                return nullptr;
        }

        const size_t row_bytes = (stride > 0) ? static_cast<size_t>(stride) : static_cast<size_t>(width * channels);
        cv::Mat raw_mat(height, width, cv_type, const_cast<uint8_t*>(buffer), row_bytes);

        cv::Mat final_mat;
        if (needs_bgr_conversion) {
            cv::cvtColor(raw_mat, final_mat, conversion_code);
        } else {
            final_mat = raw_mat.clone();
        }

        static uint64_t s_frame_counter = 1;
        auto* ctx = new FrameContext(final_mat, "capi_stream", s_frame_counter++);
        return static_cast<CurvFrameHandle>(ctx);
    } catch (...) {
        return nullptr;
    }
}

void curv_frame_destroy(CurvFrameHandle frame) {
    if (!frame) return;
    try {
        auto* ctx = static_cast<FrameContext*>(frame);
        delete ctx;
    } catch (...) {}
}

int curv_pipeline_process_frame(
    CurvPipelineHandle pipeline,
    CurvFrameHandle frame,
    char* out_json_buffer,
    int out_json_buffer_size
) {
    if (!pipeline || !frame) {
        return CURV_STATUS_INVALID_ARGUMENT;
    }

    try {
        auto* p_ctx = static_cast<PipelineContext*>(pipeline);
        auto* f_ctx = static_cast<FrameContext*>(frame);

        const auto result = p_ctx->pipeline->process(f_ctx->frame);

        p_ctx->last_quality.blur_score = result.quality.blur_score;
        p_ctx->last_quality.brightness_score = result.quality.brightness_score;
        p_ctx->last_quality.contrast_score = result.quality.contrast_score;
        p_ctx->last_quality.is_usable = result.quality.is_usable ? 1 : 0;

        nlohmann::json root_json;
        root_json["quality"] = {
            {"blur_score", result.quality.blur_score},
            {"brightness_score", result.quality.brightness_score},
            {"contrast_score", result.quality.contrast_score},
            {"is_usable", result.quality.is_usable},
            {"recommendation", result.quality.recommendation}
        };
        root_json["total_duration_us"] = result.total_duration_us;

        nlohmann::json evidence_arr = nlohmann::json::array();
        for (const auto& ev : result.evidence) {
            evidence_arr.push_back(ev.to_json());
        }

        if (p_ctx->domain_pack && result.is_usable) {
            CurvEngine::DomainInput d_in{f_ctx->frame, result.ridge_graph, result.quality};
            auto d_out = p_ctx->domain_pack->process(d_in);
            for (const auto& ev : d_out.domain_evidence) {
                evidence_arr.push_back(ev.to_json());
            }
            if (!d_out.domain_payload.empty()) {
                root_json["domain_payload"] = d_out.domain_payload;
            }
        }
        root_json["evidence"] = evidence_arr;

        p_ctx->last_json = root_json.dump();
        const int copy_status = copy_json_to_buffer(root_json, out_json_buffer, out_json_buffer_size);
        if (copy_status != CURV_STATUS_SUCCESS) {
            return copy_status;
        }

        if (!result.is_usable) {
            return CURV_STATUS_QUALITY_GATE_REJECTED;
        }

        return CURV_STATUS_SUCCESS;
    } catch (const std::exception&) {
        return CURV_STATUS_INTERNAL_ERROR;
    } catch (...) {
        return CURV_STATUS_INTERNAL_ERROR;
    }
}

int curv_pipeline_process_quad_roi(
    CurvPipelineHandle pipeline,
    CurvFrameHandle frame,
    const float* roi_corners_8_floats,
    char* out_json_buffer,
    int out_json_buffer_size
) {
    if (!pipeline || !frame || !roi_corners_8_floats) {
        return CURV_STATUS_INVALID_ARGUMENT;
    }

    try {
        auto* p_ctx = static_cast<PipelineContext*>(pipeline);
        auto* f_ctx = static_cast<FrameContext*>(frame);

        std::array<CurvEngine::Point2D, 4> corners;
        for (size_t i = 0; i < 4; ++i) {
            corners[i].x = roi_corners_8_floats[2 * i];
            corners[i].y = roi_corners_8_floats[2 * i + 1];
        }

        const int target_dim = 512;
        cv::Mat H = CurvEngine::geometry::computeQuadPerspectiveTransform(corners, target_dim);
        cv::Mat H_inv = H.inv();

        cv::Mat warped;
        cv::warpPerspective(
            f_ctx->frame.image,
            warped,
            H,
            cv::Size(target_dim, target_dim),
            cv::INTER_LINEAR,
            cv::BORDER_REPLICATE
        );

        CurvEngine::Frame warped_frame(warped, f_ctx->frame.source_id + "_roi", f_ctx->frame.timestamp_ms);

        // Dynamic parameter tuning based on diagonal
        const float diag_x = corners[2].x - corners[0].x;
        const float diag_y = corners[2].y - corners[0].y;
        const float diagonal = std::sqrt(diag_x * diag_x + diag_y * diag_y);

        auto tuned_cfg = p_ctx->param_resolver.resolve(diagonal);
        auto custom_extractor = std::make_shared<CurvEngine::ridge::StegerRidgeExtractor>(tuned_cfg);
        CurvEngine::pipeline::SyncPerceptionPipeline custom_pipeline(p_ctx->quality_analyzer, custom_extractor);

        auto result = custom_pipeline.process(warped_frame);

        p_ctx->last_quality.blur_score = result.quality.blur_score;
        p_ctx->last_quality.brightness_score = result.quality.brightness_score;
        p_ctx->last_quality.contrast_score = result.quality.contrast_score;
        p_ctx->last_quality.is_usable = result.quality.is_usable ? 1 : 0;

        // Invert curve points back to original image space and recompute true length
        for (auto& seg : result.ridge_graph.segments) {
            for (auto& pt : seg.points) {
                const double px = static_cast<double>(pt.x);
                const double py = static_cast<double>(pt.y);
                const double w = H_inv.at<double>(2, 0) * px + H_inv.at<double>(2, 1) * py + H_inv.at<double>(2, 2);
                const double inv_w = (std::abs(w) > 1e-6) ? (1.0 / w) : 1.0;
                pt.x = static_cast<float>((H_inv.at<double>(0, 0) * px + H_inv.at<double>(0, 1) * py + H_inv.at<double>(0, 2)) * inv_w);
                pt.y = static_cast<float>((H_inv.at<double>(1, 0) * px + H_inv.at<double>(1, 1) * py + H_inv.at<double>(1, 2)) * inv_w);
            }
            float new_len = 0.0f;
            for (size_t i = 1; i < seg.points.size(); ++i) {
                new_len += std::hypot(seg.points[i].x - seg.points[i - 1].x, seg.points[i].y - seg.points[i - 1].y);
            }
            seg.total_length = new_len;
        }

        nlohmann::json root_json;
        root_json["quality"] = {
            {"blur_score", result.quality.blur_score},
            {"brightness_score", result.quality.brightness_score},
            {"contrast_score", result.quality.contrast_score},
            {"is_usable", result.quality.is_usable},
            {"recommendation", result.quality.recommendation}
        };
        root_json["total_duration_us"] = result.total_duration_us;

        nlohmann::json evidence_arr = nlohmann::json::array();
        for (const auto& ev : result.evidence) {
            evidence_arr.push_back(ev.to_json());
        }

        if (p_ctx->domain_pack && result.is_usable) {
            CurvEngine::DomainInput d_in{f_ctx->frame, result.ridge_graph, result.quality};
            auto d_out = p_ctx->domain_pack->process(d_in);
            for (const auto& ev : d_out.domain_evidence) {
                evidence_arr.push_back(ev.to_json());
            }
            if (!d_out.domain_payload.empty()) {
                root_json["domain_payload"] = d_out.domain_payload;
            }
        }
        root_json["evidence"] = evidence_arr;

        p_ctx->last_json = root_json.dump();
        const int copy_status = copy_json_to_buffer(root_json, out_json_buffer, out_json_buffer_size);
        if (copy_status != CURV_STATUS_SUCCESS) {
            return copy_status;
        }

        if (!result.is_usable) {
            return CURV_STATUS_QUALITY_GATE_REJECTED;
        }

        return CURV_STATUS_SUCCESS;
    } catch (const std::exception&) {
        return CURV_STATUS_INTERNAL_ERROR;
    } catch (...) {
        return CURV_STATUS_INTERNAL_ERROR;
    }
}

int curv_pipeline_process_landmarks_roi(
    CurvPipelineHandle pipeline,
    CurvFrameHandle frame,
    const float* landmarks_42_floats,
    char* out_json_buffer,
    int out_json_buffer_size
) {
    if (!pipeline || !frame || !landmarks_42_floats) {
        return CURV_STATUS_INVALID_ARGUMENT;
    }

    try {
        auto* p_ctx = static_cast<PipelineContext*>(pipeline);
        auto* f_ctx = static_cast<FrameContext*>(frame);

        CurvEngine::geometry::HandLandmarks lms;
        lms.is_normalized = true;
        for (size_t i = 0; i < 21; ++i) {
            lms.points[i].x = landmarks_42_floats[2 * i];
            lms.points[i].y = landmarks_42_floats[2 * i + 1];
        }

        const int target_dim = 512;
        const auto transform = CurvEngine::geometry::computeCanonicalPalmTransform(
            lms,
            f_ctx->frame.width(),
            f_ctx->frame.height(),
            target_dim
        );

        cv::Mat warped = CurvEngine::geometry::warpToCanonical(
            f_ctx->frame.image,
            transform.M,
            target_dim
        );

        CurvEngine::Frame warped_frame(warped, f_ctx->frame.source_id + "_canonical_palm", f_ctx->frame.timestamp_ms);

        // Detect handedness (left vs right hand) and propagate into frame metadata for PalmZones
        const bool is_left = CurvEngine::geometry::detectIsLeftHand(lms);
        warped_frame.metadata["is_left_hand"] = is_left ? "true" : "false";

        // Adapt parameters to palm scale
        auto tuned_cfg = CurvEngine::domains::PalmParameters::computeFromLandmarks(
            lms,
            f_ctx->frame.width(),
            f_ctx->frame.height()
        );

        auto custom_extractor = std::make_shared<CurvEngine::ridge::StegerRidgeExtractor>(tuned_cfg);
        CurvEngine::pipeline::SyncPerceptionPipeline custom_pipeline(p_ctx->quality_analyzer, custom_extractor);

        auto result = custom_pipeline.process(warped_frame);

        p_ctx->last_quality.blur_score = result.quality.blur_score;
        p_ctx->last_quality.brightness_score = result.quality.brightness_score;
        p_ctx->last_quality.contrast_score = result.quality.contrast_score;
        p_ctx->last_quality.is_usable = result.quality.is_usable ? 1 : 0;

        nlohmann::json root_json;
        root_json["quality"] = {
            {"blur_score", result.quality.blur_score},
            {"brightness_score", result.quality.brightness_score},
            {"contrast_score", result.quality.contrast_score},
            {"is_usable", result.quality.is_usable},
            {"recommendation", result.quality.recommendation}
        };
        root_json["total_duration_us"] = result.total_duration_us;

        nlohmann::json evidence_arr = nlohmann::json::array();
        for (const auto& ev : result.evidence) {
            evidence_arr.push_back(ev.to_json());
        }

        // 1. First process with domain pack in 512x512 canonical space
        if (p_ctx->domain_pack && result.is_usable) {
            CurvEngine::DomainInput d_in{warped_frame, result.ridge_graph, result.quality};
            auto d_out = p_ctx->domain_pack->process(d_in);
            for (const auto& ev : d_out.domain_evidence) {
                evidence_arr.push_back(ev.to_json());
            }
            if (!d_out.domain_payload.empty()) {
                root_json["domain_payload"] = d_out.domain_payload;
            }
        }
        root_json["evidence"] = evidence_arr;

        // 2. Now invert extracted sub-pixel curves back to source image coordinates for caller visualization
        CurvEngine::geometry::invertGraphCoordinates(result.ridge_graph, transform.M_inv);

        p_ctx->last_json = root_json.dump();
        const int copy_status = copy_json_to_buffer(root_json, out_json_buffer, out_json_buffer_size);
        if (copy_status != CURV_STATUS_SUCCESS) {
            return copy_status;
        }

        if (!result.is_usable) {
            return CURV_STATUS_QUALITY_GATE_REJECTED;
        }

        return CURV_STATUS_SUCCESS;
    } catch (const std::exception&) {
        return CURV_STATUS_INTERNAL_ERROR;
    } catch (...) {
        return CURV_STATUS_INTERNAL_ERROR;
    }
}

int curv_pipeline_get_last_quality(
    CurvPipelineHandle pipeline,
    CurvQualityResult* out_quality
) {
    if (!pipeline || !out_quality) {
        return CURV_STATUS_INVALID_ARGUMENT;
    }

    try {
        auto* p_ctx = static_cast<PipelineContext*>(pipeline);
        *out_quality = p_ctx->last_quality;
        return CURV_STATUS_SUCCESS;
    } catch (...) {
        return CURV_STATUS_INTERNAL_ERROR;
    }
}

int curv_pipeline_get_last_json(
    CurvPipelineHandle pipeline,
    char* out_json_buffer,
    int out_json_buffer_size,
    int* out_required_size
) {
    if (!pipeline) {
        return CURV_STATUS_INVALID_ARGUMENT;
    }

    try {
        auto* p_ctx = static_cast<PipelineContext*>(pipeline);
        const size_t needed_len = p_ctx->last_json.length() + 1;
        if (out_required_size) {
            *out_required_size = static_cast<int>(needed_len);
        }

        if (!out_json_buffer || out_json_buffer_size <= 0) {
            return CURV_STATUS_SUCCESS;
        }

        out_json_buffer[0] = '\0';
        if (static_cast<size_t>(out_json_buffer_size) < needed_len) {
            return CURV_STATUS_BUFFER_TOO_SMALL;
        }

        std::memcpy(out_json_buffer, p_ctx->last_json.c_str(), needed_len);
        return CURV_STATUS_SUCCESS;
    } catch (...) {
        return CURV_STATUS_INTERNAL_ERROR;
    }
}

} // extern "C"

#include <gtest/gtest.h>
#include <vector>
#include <string>
#include <cstring>
#include <nlohmann/json.hpp>
#include "curv/curv_capi.h"
#include "curv_api.h"

namespace {

// High-contrast ring + bar pattern that passes the Laplacian quality gate and
// yields ridge segments in the palm ROI path.
std::vector<uint8_t> MakeRingPattern(int width, int height) {
    std::vector<uint8_t> buffer(static_cast<size_t>(width * height * 3), 40);
    for (int y = 0; y < height; ++y) {
        for (int x = 0; x < width; ++x) {
            const size_t idx = static_cast<size_t>((y * width + x) * 3);
            const float dx = static_cast<float>(x - width / 2);
            const float dy = static_cast<float>(y - height / 2);
            const float dist = std::sqrt(dx * dx + dy * dy);
            const float half_w = static_cast<float>(width) * 0.25f;
            if (std::abs(dist - half_w) < 2.0f ||
                (y >= height / 2 - 2 && y <= height / 2 + 2 && x >= width / 4 && x <= 3 * width / 4)) {
                buffer[idx + 0] = 230;
                buffer[idx + 1] = 230;
                buffer[idx + 2] = 230;
            }
        }
    }
    return buffer;
}

// 21 normalized landmarks of an upright palm (top-left origin), matching the
// schema consumed by computeCanonicalPalmTransform / PalmParameters.
std::vector<float> MakePalmLandmarks() {
    return {
        0.50f, 0.85f, // 0: Wrist
        0.38f, 0.70f, 0.30f, 0.58f, 0.25f, 0.48f, 0.22f, 0.40f, // 1-4: Thumb
        0.42f, 0.52f, 0.40f, 0.38f, 0.39f, 0.28f, 0.38f, 0.20f, // 5-8: Index
        0.50f, 0.50f, 0.50f, 0.35f, 0.50f, 0.24f, 0.50f, 0.15f, // 9-12: Middle
        0.58f, 0.52f, 0.60f, 0.38f, 0.61f, 0.28f, 0.62f, 0.21f, // 13-16: Ring
        0.66f, 0.56f, 0.69f, 0.46f, 0.71f, 0.38f, 0.72f, 0.31f  // 17-20: Pinky
    };
}

} // namespace

TEST(CapiTest, Test_EngineVersion) {
    const char* ver = curv_get_version();
    ASSERT_NE(ver, nullptr);
    EXPECT_STRNE(ver, "");
}

TEST(CapiTest, Test_PipelineLifecycleAndNullSafety) {
    // Null safety
    curv_pipeline_destroy(nullptr);
    curv_frame_destroy(nullptr);

    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);

    curv_pipeline_destroy(pipeline);
}

TEST(CapiTest, Test_FrameCreationAndProcessing) {
    CurvPipelineHandle pipeline = curv_pipeline_create("surface_inspection");
    ASSERT_NE(pipeline, nullptr);

    const int width = 200;
    const int height = 200;
    const int channels = 3;
    std::vector<uint8_t> buffer(static_cast<size_t>(width * height * channels), 40);

    // Draw high-contrast sharp curve patterns
    for (int y = 0; y < height; ++y) {
        for (int x = 0; x < width; ++x) {
            const size_t idx = static_cast<size_t>((y * width + x) * channels);
            const float dx = static_cast<float>(x - 100);
            const float dy = static_cast<float>(y - 100);
            const float dist = std::sqrt(dx * dx + dy * dy);
            // Circle ring of radius 50, thickness ~4
            if (std::abs(dist - 50.0f) < 2.0f || (y >= 98 && y <= 102 && x >= 30 && x <= 170)) {
                buffer[idx + 0] = 230; // R
                buffer[idx + 1] = 230; // G
                buffer[idx + 2] = 230; // B
            }
        }
    }

    CurvFrameHandle frame = curv_frame_create_from_buffer(
        buffer.data(),
        width,
        height,
        0, // tightly packed
        CURV_PIXEL_FORMAT_RGB
    );
    ASSERT_NE(frame, nullptr);

    // 1. Full-frame processing
    std::vector<char> json_buf(524288, 0); // 512 KB
    int status = curv_pipeline_process_frame(pipeline, frame, json_buf.data(), static_cast<int>(json_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);

    nlohmann::json parsed = nlohmann::json::parse(json_buf.data(), nullptr, false);
    EXPECT_FALSE(parsed.is_discarded());
    EXPECT_TRUE(parsed.contains("quality"));
    EXPECT_TRUE(parsed.contains("evidence"));
    EXPECT_TRUE(parsed["quality"]["is_usable"].get<bool>());

    // 2. Query quality struct
    CurvQualityResult quality_res{};
    int q_status = curv_pipeline_get_last_quality(pipeline, &quality_res);
    EXPECT_EQ(q_status, CURV_STATUS_SUCCESS);
    EXPECT_EQ(quality_res.is_usable, 1);
    EXPECT_GT(quality_res.blur_score, 0.0);

    // 3. Quad ROI processing
    const float corners[8] = {
        10.0f, 10.0f,
        190.0f, 10.0f,
        190.0f, 190.0f,
        10.0f, 190.0f
    };
    std::fill(json_buf.begin(), json_buf.end(), 0);
    status = curv_pipeline_process_quad_roi(pipeline, frame, corners, json_buf.data(), static_cast<int>(json_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);

    parsed = nlohmann::json::parse(json_buf.data(), nullptr, false);
    EXPECT_FALSE(parsed.is_discarded());

    // 4. Palm Landmarks ROI processing
    float lms[42] = {0.0f};
    // Setup Wrist (p0) and Middle MCP (p9)
    lms[0] = 0.5f; lms[1] = 0.8f; // Wrist
    lms[2 * 5] = 0.35f; lms[2 * 5 + 1] = 0.45f; // Index MCP
    lms[2 * 9] = 0.5f;  lms[2 * 9 + 1] = 0.40f; // Middle MCP
    lms[2 * 13] = 0.6f; lms[2 * 13 + 1] = 0.45f; // Ring MCP
    lms[2 * 17] = 0.7f; lms[2 * 17 + 1] = 0.50f; // Pinky MCP

    std::fill(json_buf.begin(), json_buf.end(), 0);
    status = curv_pipeline_process_landmarks_roi(pipeline, frame, lms, json_buf.data(), static_cast<int>(json_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);

    parsed = nlohmann::json::parse(json_buf.data(), nullptr, false);
    EXPECT_FALSE(parsed.is_discarded());

    // 5. Buffer too small error test
    char tiny_buf[16];
    status = curv_pipeline_process_frame(pipeline, frame, tiny_buf, sizeof(tiny_buf));
    EXPECT_EQ(status, CURV_STATUS_BUFFER_TOO_SMALL);

    // Quad and landmarks buffer too small
    status = curv_pipeline_process_quad_roi(pipeline, frame, corners, tiny_buf, sizeof(tiny_buf));
    EXPECT_EQ(status, CURV_STATUS_BUFFER_TOO_SMALL);

    status = curv_pipeline_process_landmarks_roi(pipeline, frame, lms, tiny_buf, sizeof(tiny_buf));
    EXPECT_EQ(status, CURV_STATUS_BUFFER_TOO_SMALL);

    // 6. Invalid argument tests
    EXPECT_EQ(curv_pipeline_process_frame(nullptr, frame, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_frame(pipeline, nullptr, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_frame(pipeline, frame, nullptr, 0),
              CURV_STATUS_INVALID_ARGUMENT);

    EXPECT_EQ(curv_pipeline_process_quad_roi(nullptr, frame, corners, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_quad_roi(pipeline, nullptr, corners, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_quad_roi(pipeline, frame, nullptr, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_quad_roi(pipeline, frame, corners, nullptr, 0),
              CURV_STATUS_INVALID_ARGUMENT);

    EXPECT_EQ(curv_pipeline_process_landmarks_roi(nullptr, frame, lms, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_landmarks_roi(pipeline, nullptr, lms, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_landmarks_roi(pipeline, frame, nullptr, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_landmarks_roi(pipeline, frame, lms, nullptr, 0),
              CURV_STATUS_INVALID_ARGUMENT);

    EXPECT_EQ(curv_pipeline_get_last_quality(nullptr, &quality_res), CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_get_last_quality(pipeline, nullptr), CURV_STATUS_INVALID_ARGUMENT);

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

TEST(CapiTest, QualityGateRejectionReturnsStatus) {
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);

    // Completely black flat frame fails quality gate (contrast, blur, brightness)
    const int width = 100;
    const int height = 100;
    std::vector<uint8_t> black_buf(static_cast<size_t>(width * height * 3), 0);
    CurvFrameHandle black_frame = curv_frame_create_from_buffer(
        black_buf.data(), width, height, 0, CURV_PIXEL_FORMAT_RGB);
    ASSERT_NE(black_frame, nullptr);

    std::vector<char> json_buf(1024, 0);
    int status = curv_pipeline_process_frame(pipeline, black_frame, json_buf.data(), static_cast<int>(json_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_QUALITY_GATE_REJECTED);

    const float corners[8] = {0.0f, 0.0f, 100.0f, 0.0f, 100.0f, 100.0f, 0.0f, 100.0f};
    status = curv_pipeline_process_quad_roi(pipeline, black_frame, corners, json_buf.data(), static_cast<int>(json_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_QUALITY_GATE_REJECTED);

    curv_frame_destroy(black_frame);
    curv_pipeline_destroy(pipeline);
}

TEST(CapiTest, AnthropometryAndMeshJson) {
    // 21 synthetic normalized landmarks
    const float lms[42] = {
        0.50f, 0.85f, // 0: Wrist
        0.38f, 0.70f, 0.30f, 0.58f, 0.25f, 0.48f, 0.22f, 0.40f, // 1-4: Thumb
        0.42f, 0.52f, 0.40f, 0.38f, 0.39f, 0.28f, 0.38f, 0.20f, // 5-8: Index
        0.50f, 0.50f, 0.50f, 0.35f, 0.50f, 0.24f, 0.50f, 0.15f, // 9-12: Middle
        0.58f, 0.52f, 0.60f, 0.38f, 0.61f, 0.28f, 0.62f, 0.21f, // 13-16: Ring
        0.66f, 0.56f, 0.69f, 0.46f, 0.71f, 0.38f, 0.72f, 0.31f  // 17-20: Pinky
    };

    // Invalid arguments
    std::vector<char> buf(4096, 0);
    EXPECT_EQ(curv_compute_hand_anthropometry_json(nullptr, 80.0f, nullptr, "auto", buf.data(), static_cast<int>(buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_generate_hand_mesh_json(nullptr, 80.0f, nullptr, buf.data(), static_cast<int>(buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);

    // Buffer too small
    std::vector<char> tiny_buf(10, 0);
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms, 80.0f, "{}", "auto", tiny_buf.data(), static_cast<int>(tiny_buf.size())),
              CURV_STATUS_BUFFER_TOO_SMALL);
    EXPECT_EQ(curv_generate_hand_mesh_json(lms, 80.0f, "{}", tiny_buf.data(), static_cast<int>(tiny_buf.size())),
              CURV_STATUS_BUFFER_TOO_SMALL);

    // Successful anthropometry computation
    std::vector<char> anthro_buf(16384, 0);
    int status = curv_compute_hand_anthropometry_json(lms, 80.0f, "{\"thenar\": 2.5}", "right", anthro_buf.data(), static_cast<int>(anthro_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);
    auto anthro_json = nlohmann::json::parse(anthro_buf.data());
    EXPECT_TRUE(anthro_json.contains("anthropometry"));
    EXPECT_TRUE(anthro_json["anthropometry"].contains("ratios"));
    EXPECT_TRUE(anthro_json["anthropometry"]["ratios"].contains("ratio_2d_4d"));
    EXPECT_GT(anthro_json["anthropometry"]["ratios"]["ratio_2d_4d"].get<float>(), 0.0f);

    // Successful mesh generation
    std::vector<char> mesh_buf(32768, 0);
    status = curv_generate_hand_mesh_json(lms, 80.0f, "{\"thenar\": 2.5}", mesh_buf.data(), static_cast<int>(mesh_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);
    auto mesh_json = nlohmann::json::parse(mesh_buf.data());
    EXPECT_TRUE(mesh_json.contains("mesh"));
    EXPECT_TRUE(mesh_json["mesh"].contains("vertices"));
    EXPECT_TRUE(mesh_json["mesh"].contains("triangles"));
    EXPECT_FALSE(mesh_json["mesh"]["triangles"].empty());
}

// --- Parameter variants for the pure-JSON entry points (branch coverage) -----

TEST(CapiTest, AnthropometryArgumentVariants) {
    const std::vector<float> lms = MakePalmLandmarks();
    std::vector<char> buf(16384, 0);
    const int size = static_cast<int>(buf.size());

    // Null pad-depth JSON and empty string both take the "no depths" path.
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 80.0f, nullptr, "right", buf.data(), size),
              CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 80.0f, "", "right", buf.data(), size),
              CURV_STATUS_SUCCESS);

    // Non-object JSON and non-number values are skipped, not fatal.
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 80.0f, "[1,2]", "right", buf.data(), size),
              CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 80.0f, "{\"thenar\": \"high\"}", "right", buf.data(), size),
              CURV_STATUS_SUCCESS);

    // Null hand side falls back to "auto"; non-positive reference width to 80 mm.
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 80.0f, nullptr, nullptr, buf.data(), size),
              CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 0.0f, nullptr, "left", buf.data(), size),
              CURV_STATUS_SUCCESS);

    // Malformed pad-depth JSON propagates the parse failure as INTERNAL_ERROR.
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 80.0f, "{bad", "right", buf.data(), size),
              CURV_STATUS_INTERNAL_ERROR);

    // Zero-size (non-null) output buffer is an invalid argument, not a crash.
    EXPECT_EQ(curv_compute_hand_anthropometry_json(lms.data(), 80.0f, nullptr, "right", buf.data(), 0),
              CURV_STATUS_INVALID_ARGUMENT);
}

TEST(CapiTest, MeshArgumentVariants) {
    const std::vector<float> lms = MakePalmLandmarks();
    std::vector<char> buf(32768, 0);
    const int size = static_cast<int>(buf.size());

    EXPECT_EQ(curv_generate_hand_mesh_json(lms.data(), 80.0f, nullptr, buf.data(), size), CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_generate_hand_mesh_json(lms.data(), 0.0f, "", buf.data(), size), CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_generate_hand_mesh_json(lms.data(), 80.0f, "{\"venus\": 2.0, \"bad\": null}", buf.data(), size),
              CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_generate_hand_mesh_json(lms.data(), 80.0f, "{bad", buf.data(), size), CURV_STATUS_INTERNAL_ERROR);
}

// --- Pipeline creation: domain alias branches --------------------------------

TEST(CapiTest, PipelineCreateDomainAliases) {
    // Unknown domain still yields a working pipeline without a domain pack.
    CurvPipelineHandle unknown = curv_pipeline_create("nonexistent_domain");
    ASSERT_NE(unknown, nullptr);
    curv_pipeline_destroy(unknown);

    // Null domain id is accepted (defaults to no domain pack).
    CurvPipelineHandle null_domain = curv_pipeline_create(nullptr);
    ASSERT_NE(null_domain, nullptr);
    curv_pipeline_destroy(null_domain);

    // Long-form alias selects the palm pack; "vastu" selects the floor-plan pack.
    CurvPipelineHandle palm_long = curv_pipeline_create("org.curv.domain.palm");
    ASSERT_NE(palm_long, nullptr);
    CurvPipelineHandle vastu = curv_pipeline_create("vastu");
    ASSERT_NE(vastu, nullptr);
    curv_pipeline_destroy(palm_long);
    curv_pipeline_destroy(vastu);
}

// --- Frame creation: remaining pixel formats and stride edges ----------------

TEST(CapiTest, FrameCreateGrayscaleAndAlphaFormats) {
    const int w = 64, h = 64;

    std::vector<uint8_t> gray(static_cast<size_t>(w * h), 90);
    CurvFrameHandle g = curv_frame_create_from_buffer(gray.data(), w, h, 0, CURV_PIXEL_FORMAT_GRAYSCALE);
    ASSERT_NE(g, nullptr);
    curv_frame_destroy(g);

    std::vector<uint8_t> rgba(static_cast<size_t>(w * h * 4), 60);
    for (size_t i = 3; i < rgba.size(); i += 4) rgba[i] = 255;
    CurvFrameHandle ra = curv_frame_create_from_buffer(rgba.data(), w, h, 0, CURV_PIXEL_FORMAT_RGBA);
    ASSERT_NE(ra, nullptr);
    curv_frame_destroy(ra);

    CurvFrameHandle ba = curv_frame_create_from_buffer(rgba.data(), w, h, 0, CURV_PIXEL_FORMAT_BGRA);
    ASSERT_NE(ba, nullptr);
    curv_frame_destroy(ba);

    // Positive stride equal to the minimum row bytes is accepted.
    std::vector<uint8_t> bgr(static_cast<size_t>(w * h * 3), 70);
    CurvFrameHandle exact =
        curv_frame_create_from_buffer(bgr.data(), w, h, w * 3, CURV_PIXEL_FORMAT_BGR);
    ASSERT_NE(exact, nullptr);
    curv_frame_destroy(exact);
}

// --- get_last_json: full size-query / exact-fit contract ----------------------

TEST(CapiTest, GetLastJsonSizeContract) {
    const int width = 200, height = 200;
    auto buffer = MakeRingPattern(width, height);
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);
    CurvFrameHandle frame = curv_frame_create_from_buffer(buffer.data(), width, height, 0, CURV_PIXEL_FORMAT_RGB);
    ASSERT_NE(frame, nullptr);

    std::vector<char> json_buf(524288, 0);
    ASSERT_EQ(curv_pipeline_process_frame(pipeline, frame, json_buf.data(), static_cast<int>(json_buf.size())),
              CURV_STATUS_SUCCESS);

    // 1. Size query with null buffer succeeds and reports the required length.
    int required = 0;
    EXPECT_EQ(curv_pipeline_get_last_json(pipeline, nullptr, 0, &required), CURV_STATUS_SUCCESS);
    EXPECT_GT(required, 1);

    // 2. Undersized buffer reports BUFFER_TOO_SMALL (and still updates required).
    int required2 = 0;
    std::vector<char> small(static_cast<size_t>(required) - 1, 0);
    EXPECT_EQ(curv_pipeline_get_last_json(pipeline, small.data(), static_cast<int>(small.size()), &required2),
              CURV_STATUS_BUFFER_TOO_SMALL);
    EXPECT_EQ(required2, required);

    // 3. Exact-fit buffer copies the full JSON and is NUL-terminated.
    std::vector<char> exact(static_cast<size_t>(required), 0);
    EXPECT_EQ(curv_pipeline_get_last_json(pipeline, exact.data(), required, nullptr), CURV_STATUS_SUCCESS);
    EXPECT_STREQ(exact.data(), json_buf.data());

    EXPECT_EQ(curv_pipeline_get_last_json(nullptr, nullptr, 0, nullptr), CURV_STATUS_INVALID_ARGUMENT);

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

// --- Landmarks ROI extended (minor ridges) ------------------------------------

TEST(CapiTest, LandmarksRoiExtendedFlagOffMatchesBase) {
    const int width = 200, height = 200;
    auto buffer = MakeRingPattern(width, height);
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);
    CurvFrameHandle frame = curv_frame_create_from_buffer(buffer.data(), width, height, 0, CURV_PIXEL_FORMAT_RGB);
    ASSERT_NE(frame, nullptr);
    const std::vector<float> lms = MakePalmLandmarks();

    std::vector<char> buf(1048576, 0);
    const int size = static_cast<int>(buf.size());
    const int base = curv_pipeline_process_landmarks_roi(pipeline, frame, lms.data(), buf.data(), size);
    ASSERT_TRUE(base == CURV_STATUS_SUCCESS || base == CURV_STATUS_QUALITY_GATE_REJECTED);

    // include_minor_ridges = 0 short-circuits to the base result.
    std::vector<char> buf2(1048576, 0);
    const int ext_off = curv_pipeline_process_landmarks_roi_extended(pipeline, frame, lms.data(), 0, buf2.data(), size);
    EXPECT_EQ(ext_off, base);

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

TEST(CapiTest, LandmarksRoiExtendedWithMinorRidges) {
    const int width = 200, height = 200;
    auto buffer = MakeRingPattern(width, height);
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);
    CurvFrameHandle frame = curv_frame_create_from_buffer(buffer.data(), width, height, 0, CURV_PIXEL_FORMAT_RGB);
    ASSERT_NE(frame, nullptr);
    const std::vector<float> lms = MakePalmLandmarks();

    std::vector<char> buf(1048576, 0);
    const int size = static_cast<int>(buf.size());
    const int status = curv_pipeline_process_landmarks_roi_extended(pipeline, frame, lms.data(), 1, buf.data(), size);
    ASSERT_TRUE(status == CURV_STATUS_SUCCESS || status == CURV_STATUS_QUALITY_GATE_REJECTED);

    auto parsed = nlohmann::json::parse(buf.data(), nullptr, false);
    EXPECT_FALSE(parsed.is_discarded());
    EXPECT_TRUE(parsed.contains("domain_payload"));
    EXPECT_TRUE(parsed["domain_payload"].contains("minor_ridges"));

    // Invalid pipeline handle propagates the base path's invalid-argument status.
    EXPECT_EQ(curv_pipeline_process_landmarks_roi_extended(nullptr, frame, lms.data(), 1, buf.data(), size),
              CURV_STATUS_INVALID_ARGUMENT);

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

// --- Fingertip classification --------------------------------------------------

TEST(CapiTest, ClassifyFingertipRoi) {
    const int width = 200, height = 200;
    auto buffer = MakeRingPattern(width, height);
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);
    CurvFrameHandle frame = curv_frame_create_from_buffer(buffer.data(), width, height, 0, CURV_PIXEL_FORMAT_RGB);
    ASSERT_NE(frame, nullptr);
    const std::vector<float> lms = MakePalmLandmarks();

    std::vector<char> buf(65536, 0);
    const int size = static_cast<int>(buf.size());

    EXPECT_EQ(curv_classify_fingertip_roi(pipeline, frame, lms.data(), nullptr, buf.data(), size),
              CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_classify_fingertip_roi(nullptr, frame, lms.data(), "index", buf.data(), size),
              CURV_STATUS_INVALID_ARGUMENT);

    const int status = curv_classify_fingertip_roi(pipeline, frame, lms.data(), "index", buf.data(), size);
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);
    auto parsed = nlohmann::json::parse(buf.data(), nullptr, false);
    EXPECT_FALSE(parsed.is_discarded());
    EXPECT_TRUE(parsed.contains("domain_payload"));
    EXPECT_TRUE(parsed["domain_payload"].contains("fingertip_pattern"));

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

// --- Orbit session analysis ------------------------------------------------------

TEST(CapiTest, AnalyzeOrbitSession) {
    std::vector<char> buf(65536, 0);
    const int size = static_cast<int>(buf.size());

    // Null session string is an invalid argument.
    EXPECT_EQ(curv_analyze_orbit_session_json(nullptr, buf.data(), size), CURV_STATUS_INVALID_ARGUMENT);

    // Malformed JSON is an internal error, not a crash.
    EXPECT_EQ(curv_analyze_orbit_session_json("{bad json", buf.data(), size), CURV_STATUS_INTERNAL_ERROR);

    // Minimal but well-formed orbit session (schema curv_orbit_session_v1 subset).
    std::vector<float> lm = MakePalmLandmarks();
    nlohmann::json landmarks = nlohmann::json::array();
    for (size_t i = 0; i < 21; ++i) {
        landmarks.push_back({lm[2 * i], lm[2 * i + 1]});
    }
    nlohmann::json session = {
        {"reference_gravity", {0.0, 0.0, -1.0}},
        {"metric", {{"palm_width_mm", 80.0}}},
        {"frames", nlohmann::json::array({
            {{"bin", "flat_center"}, {"device_roll_deg", 0.0}, {"gravity", {0.0, 0.0, -1.0}},
             {"landmarks", landmarks}, {"image_width", 1000}, {"image_height", 1000},
             {"depth_mm", {{"venus", 2.0}, {"luna", 1.5}}}},
            {{"bin", "tilt_left"}, {"device_roll_deg", 12.0}, {"gravity", {0.2, 0.0, -0.98}},
             {"landmarks", landmarks}, {"image_width", 1000}, {"image_height", 1000}},
        })}};
    const std::string session_str = session.dump();

    const int status =
        curv_analyze_orbit_session_json(session_str.c_str(), buf.data(), size);
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);
    auto parsed = nlohmann::json::parse(buf.data(), nullptr, false);
    EXPECT_FALSE(parsed.is_discarded());
    ASSERT_TRUE(parsed.contains("domain_payload"));
    ASSERT_TRUE(parsed["domain_payload"].contains("mount_relief"));
}

// --- Degenerate quad ROI corners stay inside the ABI contract -----------------

TEST(CapiTest, QuadRoiDegenerateCornersReturnDefinedStatus) {
    const int width = 200, height = 200;
    auto buffer = MakeRingPattern(width, height);
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);
    CurvFrameHandle frame = curv_frame_create_from_buffer(buffer.data(), width, height, 0, CURV_PIXEL_FORMAT_RGB);
    ASSERT_NE(frame, nullptr);

    std::vector<char> buf(1048576, 0);
    const int size = static_cast<int>(buf.size());

    // Degenerate inputs (collinear, duplicate, inverted corners) must never
    // throw across the C boundary: they resolve to a defined status code.
    const float collinear[8] = {10.0f, 100.0f, 60.0f, 100.0f, 120.0f, 100.0f, 180.0f, 100.0f};
    const float duplicate[8] = {10.0f, 10.0f, 10.0f, 10.0f, 190.0f, 190.0f, 10.0f, 190.0f};
    const float inverted[8] = {190.0f, 10.0f, 10.0f, 10.0f, 190.0f, 190.0f, 10.0f, 190.0f};
    for (const float* corners : {collinear, duplicate, inverted}) {
        const int status = curv_pipeline_process_quad_roi(pipeline, frame, corners, buf.data(), size);
        EXPECT_TRUE(status == CURV_STATUS_SUCCESS || status == CURV_STATUS_QUALITY_GATE_REJECTED ||
                    status == CURV_STATUS_BUFFER_TOO_SMALL || status == CURV_STATUS_INTERNAL_ERROR)
            << "unexpected status " << status;
    }

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

// --- Zero-size output buffer on the main process path ---------------------------

TEST(CapiTest, ProcessFrameZeroSizeBufferIsInvalidArgument) {
    const int width = 64, height = 64;
    auto buffer = MakeRingPattern(width, height);
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);
    CurvFrameHandle frame = curv_frame_create_from_buffer(buffer.data(), width, height, 0, CURV_PIXEL_FORMAT_RGB);
    ASSERT_NE(frame, nullptr);

    std::vector<char> buf(64, 0);
    EXPECT_EQ(curv_pipeline_process_frame(pipeline, frame, buf.data(), 0), CURV_STATUS_INVALID_ARGUMENT);
    EXPECT_EQ(curv_pipeline_process_frame(pipeline, frame, nullptr, static_cast<int>(buf.size())),
              CURV_STATUS_INVALID_ARGUMENT);

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

TEST(CapiTest, SetPreprocessingToggle) {
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);
    EXPECT_EQ(curv_pipeline_set_preprocessing(pipeline, 0), CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_pipeline_set_preprocessing(pipeline, 1), CURV_STATUS_SUCCESS);
    EXPECT_EQ(curv_pipeline_set_preprocessing(nullptr, 0), CURV_STATUS_INVALID_ARGUMENT);
    curv_pipeline_destroy(pipeline);
}

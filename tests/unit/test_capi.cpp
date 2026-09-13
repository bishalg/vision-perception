#include <gtest/gtest.h>
#include <vector>
#include <string>
#include <cstring>
#include <nlohmann/json.hpp>
#include "curv/curv_capi.h"
#include "curv_api.h"

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

#include <gtest/gtest.h>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>
#include <opencv2/core.hpp>
#include "curv/adapters/HandLandmarkDetector.hpp"

// -----------------------------------------------------------------------------
// The DNN inference path in HandLandmarkDetector had no test coverage because
// the repo ships no ONNX hand model. These tests synthesize a minimal valid
// ONNX graph (Identity over a float initializer) by hand-encoding the protobuf
// wire format, so model loading, forward inference, output decoding (both
// stride variants + coordinate clamping) and the heuristic fallback are all
// exercised without any external asset.
// -----------------------------------------------------------------------------

namespace {

void put_varint(std::vector<uint8_t>& out, uint64_t v) {
    while (v >= 0x80) {
        out.push_back(static_cast<uint8_t>((v & 0x7F) | 0x80));
        v >>= 7;
    }
    out.push_back(static_cast<uint8_t>(v));
}

void put_tag(std::vector<uint8_t>& out, int field, int wire_type) {
    put_varint(out, (static_cast<uint64_t>(field) << 3) | static_cast<uint64_t>(wire_type));
}

void put_len(std::vector<uint8_t>& out, int field, const std::vector<uint8_t>& payload) {
    put_tag(out, field, 2);
    put_varint(out, payload.size());
    out.insert(out.end(), payload.begin(), payload.end());
}

void put_str(std::vector<uint8_t>& out, int field, const std::string& s) {
    std::vector<uint8_t> const bytes(s.begin(), s.end());
    put_len(out, field, bytes);
}

void put_varint_field(std::vector<uint8_t>& out, int field, uint64_t v) {
    put_tag(out, field, 0);
    put_varint(out, v);
}

// ValueInfoProto: name + FLOAT tensor type with the given shape.
std::vector<uint8_t> make_value_info(const std::string& name, const std::vector<int64_t>& dims) {
    std::vector<uint8_t> shape;
    for (int64_t d : dims) {
        std::vector<uint8_t> dim;
        put_varint_field(dim, 1, static_cast<uint64_t>(d));  // Dimension.dim_value
        put_len(shape, 1, dim);                              // TensorShapeProto.dim
    }
    std::vector<uint8_t> tensor_type;
    put_varint_field(tensor_type, 1, 1);  // elem_type = FLOAT
    put_len(tensor_type, 2, shape);       // shape
    std::vector<uint8_t> type;
    put_len(type, 1, tensor_type);        // TypeProto.tensor_type
    std::vector<uint8_t> info;
    put_str(info, 1, name);               // ValueInfoProto.name
    put_len(info, 2, type);               // ValueInfoProto.type
    return info;
}

// Builds a minimal ONNX ModelProto: x<declared input> -> Mul(x, K) -> y.
// The detector feeds a 1x3x224x224 blob (pixels scaled to 0…1); with a uniform
// image every output element equals K * pixel/255, and the decoder divides by
// 224 — so K controls whether decoded coordinates clamp high/low or pass
// through in-range. The declared graph input is the only input, which keeps
// OpenCV 5's setInput("") counting happy (initializers alone do not).
std::vector<uint8_t> make_mul_onnx(const std::vector<int64_t>& dims, float k) {
    // TensorProto scalar initializer "K"
    std::vector<uint8_t> k_tensor;
    put_varint_field(k_tensor, 2, 1);  // data_type = FLOAT
    put_str(k_tensor, 8, "K");
    std::vector<uint8_t> raw(sizeof(float), 0);
    std::memcpy(raw.data(), &k, sizeof(float));
    put_len(k_tensor, 9, raw);  // raw_data

    // NodeProto: Mul(x, K) -> y (inputs are repeated field 1)
    std::vector<uint8_t> node;
    put_str(node, 1, "x");
    put_str(node, 1, "K");
    put_str(node, 2, "y");
    put_str(node, 4, "Mul");

    std::vector<uint8_t> graph;
    put_len(graph, 1, node);                         // node
    put_str(graph, 2, "curv_test_graph");            // name
    put_len(graph, 5, k_tensor);                     // initializer
    put_len(graph, 11, make_value_info("x", dims));  // input
    put_len(graph, 12, make_value_info("y", dims));  // output

    // OperatorSetIdProto { version = 13 }
    std::vector<uint8_t> opset;
    put_varint_field(opset, 2, 13);

    // ModelProto
    std::vector<uint8_t> model;
    put_varint_field(model, 1, 3);   // ir_version
    put_str(model, 2, "curv-test");  // producer_name
    put_len(model, 7, graph);        // graph
    put_len(model, 8, opset);        // opset_import
    return model;
}

// Builds a minimal ONNX ModelProto: x<declared input> -> GlobalAveragePool -> y.
// OpenCV re-infers output shape from the actual input blob, so a reducing op is
// the only way to get a short output: pooling the 1x3x224x224 blob yields
// [1,3,1,1] = 3 elements, forcing the detector's stride-2 decode path.
std::vector<uint8_t> make_gap_onnx(const std::vector<int64_t>& dims_in, const std::vector<int64_t>& dims_out) {
    std::vector<uint8_t> node;
    put_str(node, 1, "x");                   // input
    put_str(node, 2, "y");                   // output
    put_str(node, 4, "GlobalAveragePool");   // op_type

    std::vector<uint8_t> graph;
    put_len(graph, 1, node);                              // node
    put_str(graph, 2, "curv_test_graph");                 // name
    put_len(graph, 11, make_value_info("x", dims_in));    // input
    put_len(graph, 12, make_value_info("y", dims_out));   // output

    std::vector<uint8_t> opset;
    put_varint_field(opset, 2, 13);

    std::vector<uint8_t> model;
    put_varint_field(model, 1, 3);   // ir_version
    put_str(model, 2, "curv-test");  // producer_name
    put_len(model, 7, graph);        // graph
    put_len(model, 8, opset);        // opset_import
    return model;
}

std::string write_temp_model(const std::string& name, const std::vector<uint8_t>& bytes) {
    const auto path = std::filesystem::temp_directory_path() / name;
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    out.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
    return path.string();
}

// Uniform gray value used by make_test_image; blob scales it by 1/255.
constexpr float kImageValue = 100.0f;
constexpr float kBlobValue = kImageValue / 255.0f;  // 0.39216…
// Decoded coordinate = blob_value * K / 224 (detector divides by input size).
constexpr float kClampHighK = 57120.0f;  // 0.39216 * 57120 / 224 = 100 -> clamp to 1
constexpr float kClampLowK = -57120.0f;  // -100 -> clamp to 0
constexpr float kInRangeK = 280.0f;      // 0.4902 -> passes through

cv::Mat make_test_image(int w = 96, int h = 96) {
    return cv::Mat(h, w, CV_8UC3, cv::Scalar(kImageValue, kImageValue, kImageValue));
}

} // namespace

TEST(HandLandmarkDetectorTest, LoadModelRejectsGarbageFile) {
    CurvEngine::adapters::HandLandmarkDetector detector;
    EXPECT_FALSE(detector.loadModel("/curv/definitely/missing/model.onnx"));
    EXPECT_FALSE(detector.isModelLoaded());

    const auto garbage = write_temp_model("curv_garbage.onnx", {0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01});
    EXPECT_FALSE(detector.loadModel(garbage));
    EXPECT_FALSE(detector.isModelLoaded());
}

TEST(HandLandmarkDetectorTest, LoadModelSucceedsOnMinimalOnnx) {
    const auto model = write_temp_model(
        "curv_hand_mul.onnx", make_mul_onnx({1, 3, 224, 224}, kClampHighK));

    CurvEngine::adapters::HandLandmarkDetector detector;
    ASSERT_TRUE(detector.loadModel(model));
    EXPECT_TRUE(detector.isModelLoaded());

    const auto lms = detector.detect(make_test_image());
    ASSERT_TRUE(lms.has_value());
    EXPECT_EQ(lms->points.size(), static_cast<size_t>(21));
    EXPECT_TRUE(lms->is_normalized);
    // Every decoded coordinate exceeds 1 and clamps to the unit square corner.
    EXPECT_FLOAT_EQ(lms->points[0].x, 1.0f);
    EXPECT_FLOAT_EQ(lms->points[0].y, 1.0f);
    EXPECT_FLOAT_EQ(lms->points[20].x, 1.0f);
}

TEST(HandLandmarkDetectorTest, DetectInRangePassthrough) {
    const auto model = write_temp_model(
        "curv_hand_mul_range.onnx", make_mul_onnx({1, 3, 224, 224}, kInRangeK));

    CurvEngine::adapters::HandLandmarkDetector detector;
    ASSERT_TRUE(detector.loadModel(model));

    const auto lms = detector.detect(make_test_image());
    ASSERT_TRUE(lms.has_value());
    EXPECT_NEAR(lms->points[1].x, kBlobValue * kInRangeK / 224.0f, 1e-4f);
    EXPECT_NEAR(lms->points[12].y, kBlobValue * kInRangeK / 224.0f, 1e-4f);
}

TEST(HandLandmarkDetectorTest, DetectClampsLowCoordinates) {
    const auto model = write_temp_model(
        "curv_hand_mul_low.onnx", make_mul_onnx({1, 3, 224, 224}, kClampLowK));

    CurvEngine::adapters::HandLandmarkDetector detector;
    ASSERT_TRUE(detector.loadModel(model));

    const auto lms = detector.detect(make_test_image());
    ASSERT_TRUE(lms.has_value());
    EXPECT_FLOAT_EQ(lms->points[5].x, 0.0f);
    EXPECT_FLOAT_EQ(lms->points[5].y, 0.0f);
}

TEST(HandLandmarkDetectorTest, DetectShortOutputUsesStride2) {
    // Pooled output has 3 elements < 63 -> decoder falls back to the (x, y)
    // stride-2 layout, and only landmark 0 has data; the rest keep defaults.
    const auto model =
        write_temp_model("curv_hand_gap.onnx", make_gap_onnx({1, 3, 224, 224}, {1, 3, 1, 1}));

    CurvEngine::adapters::HandLandmarkDetector detector;
    ASSERT_TRUE(detector.loadModel(model));

    const auto lms = detector.detect(make_test_image());
    ASSERT_TRUE(lms.has_value());
    // Landmark 0 decodes the pooled average blob value (0.39216) / 224.
    EXPECT_NEAR(lms->points[0].x, kBlobValue / 224.0f, 1e-5f);
    // Landmarks 1..20 lack elements in the short output -> left at defaults.
    EXPECT_FLOAT_EQ(lms->points[20].x, 0.0f);
    EXPECT_FLOAT_EQ(lms->points[20].y, 0.0f);
}

TEST(HandLandmarkDetectorTest, DetectEmptyImageReturnsNullopt) {
    CurvEngine::adapters::HandLandmarkDetector detector;
    EXPECT_FALSE(detector.detect(cv::Mat()).has_value());
    // Also with a model loaded — the empty-image guard runs first.
    const auto model =
        write_temp_model("curv_hand_mul2.onnx", make_mul_onnx({1, 3, 224, 224}, kInRangeK));
    ASSERT_TRUE(detector.loadModel(model));
    EXPECT_FALSE(detector.detect(cv::Mat()).has_value());
}

TEST(HandLandmarkDetectorTest, DetectWithoutModelUsesHeuristic) {
    CurvEngine::adapters::HandLandmarkDetector detector;
    ASSERT_FALSE(detector.isModelLoaded());

    const auto lms = detector.detect(make_test_image());
    ASSERT_TRUE(lms.has_value());
    EXPECT_EQ(lms->points.size(), static_cast<size_t>(21));
    // Heuristic palms are normalized landmarks inside the unit square.
    for (const auto& pt : lms->points) {
        EXPECT_GE(pt.x, 0.0f);
        EXPECT_LE(pt.x, 1.0f);
        EXPECT_GE(pt.y, 0.0f);
        EXPECT_LE(pt.y, 1.0f);
    }
    // Wrist (0) sits at the bottom of the palm; middle fingertip (12) above it
    // (normalized image space is top-left origin, so "above" = smaller y).
    EXPECT_LT(lms->points[12].y, lms->points[0].y);
}

TEST(HandLandmarkDetectorTest, ConstructorLoadsModelDirectly) {
    const auto model =
        write_temp_model("curv_hand_ctor.onnx", make_mul_onnx({1, 3, 224, 224}, kInRangeK));
    CurvEngine::adapters::HandLandmarkDetector detector(model);
    EXPECT_TRUE(detector.isModelLoaded());

    // The convenience constructor with an empty path stays unloaded.
    CurvEngine::adapters::HandLandmarkDetector empty({});
    EXPECT_FALSE(empty.isModelLoaded());
}

TEST(HandLandmarkDetectorTest, HeuristicLeftHandMirrorsX) {
    const auto right = CurvEngine::adapters::HandLandmarkDetector::generateHeuristicLandmarks(200, 200, false);
    const auto left = CurvEngine::adapters::HandLandmarkDetector::generateHeuristicLandmarks(200, 200, true);
    for (size_t i = 0; i < left.points.size(); ++i) {
        EXPECT_FLOAT_EQ(left.points[i].x, 1.0f - right.points[i].x) << "landmark " << i;
        EXPECT_FLOAT_EQ(left.points[i].y, right.points[i].y) << "landmark " << i;
    }
}

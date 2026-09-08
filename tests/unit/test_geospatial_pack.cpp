#include <gtest/gtest.h>
#include <opencv2/core.hpp>

#include "curv/Frame.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/QualityReport.hpp"
#include "curv/domains/GeospatialFloodPack.hpp"

namespace {

using namespace CurvEngine;

class GeospatialPackTest : public ::testing::Test {
protected:
    Frame dummy_frame{cv::Mat(100, 100, CV_8UC3, cv::Scalar(128, 128, 128)), "test", 0};
    QualityReport nominal_quality{50.0, 128.0, 45.0, true, "Nominal"};


    CurveSegment makeSegment(int id, float length) {
        CurveSegment seg;
        seg.id = id;
        seg.total_length = length;
        seg.points.emplace_back(0.0f, 0.0f);
        seg.points.emplace_back(length, 0.0f);
        return seg;
    }
};

TEST_F(GeospatialPackTest, DomainMetadataCorrect) {
    domains::GeospatialFloodPack pack;
    EXPECT_EQ(pack.name(), "org.curv.domain.geospatial");
    EXPECT_EQ(pack.version(), "0.1.0");

    std::unique_ptr<IDomainPack> polymorphic = std::make_unique<domains::GeospatialFloodPack>();
    ASSERT_NE(polymorphic, nullptr);
    polymorphic.reset();
}

TEST_F(GeospatialPackTest, EmptyGraphReturnsZeroCounts) {
    domains::GeospatialFloodPack pack;
    RidgeGraph graph;
    DomainInput input{dummy_frame, graph, nominal_quality};

    const auto output = pack.process(input);
    EXPECT_EQ(output.domain_name, "org.curv.domain.geospatial");
    EXPECT_TRUE(output.domain_evidence.empty());
    EXPECT_EQ(output.domain_payload["total_features"].get<int>(), 0);
    EXPECT_EQ(output.domain_payload["river_channel_count"].get<int>(), 0);
    EXPECT_EQ(output.domain_payload["tributary_count"].get<int>(), 0);
    EXPECT_EQ(output.domain_payload["minor_feature_count"].get<int>(), 0);
    EXPECT_DOUBLE_EQ(output.domain_payload["total_feature_length_px"].get<double>(), 0.0);
}

TEST_F(GeospatialPackTest, LongSegmentClassifiedAsRiverChannel) {
    domains::GeospatialFloodPack pack;
    RidgeGraph graph;
    graph.segments.push_back(makeSegment(1, 220.0f));
    DomainInput input{dummy_frame, graph, nominal_quality};

    const auto output = pack.process(input);
    ASSERT_EQ(output.domain_evidence.size(), 1u);
    EXPECT_EQ(output.domain_evidence[0].type, "river_channel");
    EXPECT_DOUBLE_EQ(output.domain_evidence[0].confidence, 0.95);
    EXPECT_EQ(output.domain_payload["river_channel_count"].get<int>(), 1);
    EXPECT_EQ(output.domain_payload["tributary_count"].get<int>(), 0);
    EXPECT_EQ(output.domain_payload["minor_feature_count"].get<int>(), 0);
}

TEST_F(GeospatialPackTest, TributarySegmentClassifiedAsTributary) {
    domains::GeospatialFloodPack pack;
    RidgeGraph graph;
    graph.segments.push_back(makeSegment(1, 85.0f));
    DomainInput input{dummy_frame, graph, nominal_quality};

    const auto output = pack.process(input);
    ASSERT_EQ(output.domain_evidence.size(), 1u);
    EXPECT_EQ(output.domain_evidence[0].type, "tributary");
    EXPECT_DOUBLE_EQ(output.domain_evidence[0].confidence, 0.80);
    EXPECT_EQ(output.domain_payload["tributary_count"].get<int>(), 1);
}

TEST_F(GeospatialPackTest, ShortSegmentClassifiedAsMinorFeature) {
    domains::GeospatialFloodPack pack;
    RidgeGraph graph;
    graph.segments.push_back(makeSegment(1, 25.0f));
    DomainInput input{dummy_frame, graph, nominal_quality};

    const auto output = pack.process(input);
    ASSERT_EQ(output.domain_evidence.size(), 1u);
    EXPECT_EQ(output.domain_evidence[0].type, "minor_feature");
    EXPECT_DOUBLE_EQ(output.domain_evidence[0].confidence, 0.50);
    EXPECT_EQ(output.domain_payload["minor_feature_count"].get<int>(), 1);
}

TEST_F(GeospatialPackTest, MixedSegmentsCorrectAggregateMetrics) {
    domains::GeospatialFloodPack pack;
    RidgeGraph graph;
    graph.segments.push_back(makeSegment(1, 200.0f)); // river
    graph.segments.push_back(makeSegment(2, 100.0f)); // tributary
    graph.segments.push_back(makeSegment(3, 20.0f));  // minor
    DomainInput input{dummy_frame, graph, nominal_quality};

    const auto output = pack.process(input);
    EXPECT_EQ(output.domain_evidence.size(), 3u);
    EXPECT_EQ(output.domain_payload["total_features"].get<int>(), 3);
    EXPECT_EQ(output.domain_payload["river_channel_count"].get<int>(), 1);
    EXPECT_EQ(output.domain_payload["tributary_count"].get<int>(), 1);
    EXPECT_EQ(output.domain_payload["minor_feature_count"].get<int>(), 1);
    EXPECT_FLOAT_EQ(output.domain_payload["max_feature_length_px"].get<float>(), 200.0f);
    EXPECT_FLOAT_EQ(output.domain_payload["total_feature_length_px"].get<float>(), 320.0f);
    EXPECT_NEAR(output.domain_payload["mean_feature_length_px"].get<double>(), 106.666, 0.01);
}

} // namespace

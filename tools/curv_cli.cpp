#include <iostream>
#include <string>
#include <opencv2/core.hpp>
#include <opencv2/imgcodecs.hpp>
#include "curv/Version.hpp"
#include "curv/GeometryTypes.hpp"

int main(int argc, char* argv[]) {
    // 1. Print CurvEngine version & OpenCV version
    std::cout << "==================================================" << std::endl;
    std::cout << "CurvEngine CLI Verification Tool" << std::endl;
    std::cout << "Library Version: " << CurvEngine::getVersion() << std::endl;
    std::cout << "OpenCV Version:  " << cv::getVersionString() << std::endl;
    std::cout << "==================================================" << std::endl;

    // 2. Instantiate sample CurvEngine::RidgeGraph with mock objects
    CurvEngine::Point2D p1{100.5f, 150.25f, 0.95f, 0.99f};
    CurvEngine::Point2D p2{105.0f, 155.50f, 0.92f, 0.98f};
    CurvEngine::Point2D p3{110.25f, 160.75f, 0.88f, 0.95f};

    CurvEngine::CurveSegment seg1;
    seg1.id = 101;
    seg1.points = {p1, p2, p3};
    seg1.total_length = 14.5f;
    seg1.average_curvature = 0.012f;
    seg1.is_closed = false;

    CurvEngine::Junction junc1;
    junc1.position = p3;
    junc1.connected_segment_ids = {101, 102};
    junc1.branch_angle = 0.785f; // ~45 deg in radians

    CurvEngine::RidgeGraph graph;
    graph.segments.push_back(seg1);
    graph.junctions.push_back(junc1);
    graph.debug_mask = cv::Mat::zeros(300, 300, CV_8UC1);

    std::cout << "[SANITY CHECK] CurvEngine::RidgeGraph instantiated successfully:" << std::endl;
    std::cout << "  - Segments: " << graph.segments.size()
              << " (Segment 101 points: " << graph.segments[0].points.size() << ")" << std::endl;
    std::cout << "  - Junctions: " << graph.junctions.size()
              << " (Junction branch angle: " << graph.junctions[0].branch_angle << " rad)" << std::endl;
    std::cout << "  - Debug Mask: " << graph.debug_mask.cols << "x" << graph.debug_mask.rows << std::endl;

    // 3. Optional --image <path> argument handling
    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "--image" && i + 1 < argc) {
            std::string image_path = argv[++i];
            std::cout << "\n[IMAGE LOADER] Loading image: " << image_path << std::endl;
            cv::Mat img = cv::imread(image_path, cv::IMREAD_UNCHANGED);
            if (img.empty()) {
                std::cerr << "[ERROR] Could not load image from path: " << image_path << std::endl;
                return 1;
            }
            std::cout << "[SUCCESS] Image loaded successfully!" << std::endl;
            std::cout << "  - Width:    " << img.cols << " px" << std::endl;
            std::cout << "  - Height:   " << img.rows << " px" << std::endl;
            std::cout << "  - Channels: " << img.channels() << std::endl;
        }
    }

    std::cout << "\n[PASS] curv_cli sanity check completed successfully." << std::endl;
    return 0;
}

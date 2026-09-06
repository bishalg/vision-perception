# ==============================================================================
# curv_core / vision_perception_core Static Library Target
# ==============================================================================

add_library(curv_core STATIC
    src/Version.cpp
    src/Frame.cpp
    src/QualityReport.cpp
    src/Evidence.cpp
    src/quality/LaplacianQualityAnalyzer.cpp
    src/ridge/HessianMath.cpp
    src/ridge/StegerRidgeExtractor.cpp
    src/pipeline/SyncPerceptionPipeline.cpp
    src/geometry/RoiWarper.cpp
    src/roi/CanonicalWarper.cpp
    src/config/DynamicParameterResolver.cpp
    src/geometry/ZoneIntersector.cpp
)

# Provide alias target for architectural naming flexibility
add_library(vision_perception_core ALIAS curv_core)

target_include_directories(curv_core
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
)

target_link_libraries(curv_core
    PUBLIC
        ${OpenCV_LIBS}
        nlohmann_json::nlohmann_json
        spdlog::spdlog
)

vision_set_compiler_warnings(curv_core)

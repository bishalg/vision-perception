# ==============================================================================
# tests/CMakeLists.txt - GoogleTest Suite for vision-perception
# ==============================================================================

find_package(GTest QUIET)
if(NOT GTest_FOUND)
    message(STATUS "[vision-perception] GTest not found locally. Fetching GoogleTest via FetchContent...")
    include(FetchContent)
    FetchContent_Declare(
        googletest
        GIT_REPOSITORY https://github.com/google/googletest.git
        GIT_TAG        v1.14.0
    )
    set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
    set(INSTALL_GTEST OFF CACHE BOOL "" FORCE)
    set(BUILD_GMOCK OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(googletest)
else()
    message(STATUS "[vision-perception] Found local GTest: ${GTest_DIR}")
endif()

# ------------------------------------------------------------------------------
# Unit Test: test_quality_analyzer
# ------------------------------------------------------------------------------
add_executable(test_quality_analyzer
    unit/test_quality_analyzer.cpp
)
target_link_libraries(test_quality_analyzer
    PRIVATE
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_quality_analyzer)
add_test(NAME test_quality_analyzer COMMAND test_quality_analyzer)

# ------------------------------------------------------------------------------
# Unit Test: test_steger_extractor
# ------------------------------------------------------------------------------
add_executable(test_steger_extractor
    unit/test_steger_extractor.cpp
)
target_link_libraries(test_steger_extractor
    PRIVATE
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_steger_extractor)
add_test(NAME test_steger_extractor COMMAND test_steger_extractor)

# ------------------------------------------------------------------------------
# Unit Test: test_palm_domain
# ------------------------------------------------------------------------------
add_executable(test_palm_domain
    unit/test_palm_domain.cpp
)
target_link_libraries(test_palm_domain
    PRIVATE
        curv_domains
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_palm_domain)
add_test(NAME test_palm_domain COMMAND test_palm_domain)

# ------------------------------------------------------------------------------
# Unit Test: test_surface_inspection
# ------------------------------------------------------------------------------
add_executable(test_surface_inspection
    unit/test_surface_inspection.cpp
)
target_link_libraries(test_surface_inspection
    PRIVATE
        curv_domains
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_surface_inspection)
add_test(NAME test_surface_inspection COMMAND test_surface_inspection)

# ------------------------------------------------------------------------------
# Unit Test: test_geospatial_pack
# ------------------------------------------------------------------------------
add_executable(test_geospatial_pack
    unit/test_geospatial_pack.cpp
)
target_link_libraries(test_geospatial_pack
    PRIVATE
        curv_domains
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_geospatial_pack)
add_test(NAME test_geospatial_pack COMMAND test_geospatial_pack)


# ------------------------------------------------------------------------------
# Unit Test: test_overlay_renderer
# ------------------------------------------------------------------------------
add_executable(test_overlay_renderer
    unit/test_overlay_renderer.cpp
)
target_link_libraries(test_overlay_renderer
    PRIVATE
        curv_adapters
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_overlay_renderer)
add_test(NAME test_overlay_renderer COMMAND test_overlay_renderer)

# ------------------------------------------------------------------------------
# Unit Test: test_hand_boundary_detector
# ------------------------------------------------------------------------------
add_executable(test_hand_boundary_detector
    unit/test_hand_boundary_detector.cpp
)
target_link_libraries(test_hand_boundary_detector
    PRIVATE
        curv_adapters
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_hand_boundary_detector)
add_test(NAME test_hand_boundary_detector COMMAND test_hand_boundary_detector)

# ------------------------------------------------------------------------------
# Unit Test: test_roi_warper
# ------------------------------------------------------------------------------
add_executable(test_roi_warper
    unit/test_roi_warper.cpp
)
target_link_libraries(test_roi_warper
    PRIVATE
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_roi_warper)
add_test(NAME test_roi_warper COMMAND test_roi_warper)

# ------------------------------------------------------------------------------
# Unit Test: test_capi
# ------------------------------------------------------------------------------
add_executable(test_capi
    unit/test_capi.cpp
)
target_link_libraries(test_capi
    PRIVATE
        curv_capi
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_capi)
add_test(NAME test_capi COMMAND test_capi)

# ------------------------------------------------------------------------------
# Unit Test: test_zone_intersector
# ------------------------------------------------------------------------------
add_executable(test_zone_intersector
    unit/test_zone_intersector.cpp
)
target_link_libraries(test_zone_intersector
    PRIVATE
        curv_domains
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_zone_intersector)
add_test(NAME test_zone_intersector COMMAND test_zone_intersector)

# ------------------------------------------------------------------------------
# Unit Test: test_vlm_formatter
# ------------------------------------------------------------------------------
add_executable(test_vlm_formatter
    unit/test_vlm_formatter.cpp
)
target_link_libraries(test_vlm_formatter
    PRIVATE
        curv_domains
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_vlm_formatter)
add_test(NAME test_vlm_formatter COMMAND test_vlm_formatter)

# ------------------------------------------------------------------------------
# Unit Test: test_geometry_types
# ------------------------------------------------------------------------------
add_executable(test_geometry_types
    unit/test_geometry_types.cpp
)
target_link_libraries(test_geometry_types
    PRIVATE
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_geometry_types)
add_test(NAME test_geometry_types COMMAND test_geometry_types)

# ------------------------------------------------------------------------------
# Unit Test: test_regression_audit
# ------------------------------------------------------------------------------
add_executable(test_regression_audit
    unit/test_regression_audit.cpp
)
target_link_libraries(test_regression_audit
    PRIVATE
        curv_capi
        curv_domains
        curv_adapters
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_regression_audit)
add_test(NAME test_regression_audit COMMAND test_regression_audit)

# ------------------------------------------------------------------------------
# Unit Test: test_evaluate_lib (Phase B accuracy evaluator, TDD)
# ------------------------------------------------------------------------------
add_executable(test_evaluate_lib
    unit/test_evaluate_lib.cpp
)
target_link_libraries(test_evaluate_lib
    PRIVATE
        curv_tools_lib
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_evaluate_lib)
add_test(NAME test_evaluate_lib COMMAND test_evaluate_lib)

# ------------------------------------------------------------------------------
# Unit Test: test_synth_lib (Phase B synthetic ground-truth generator, TDD)
# ------------------------------------------------------------------------------
add_executable(test_synth_lib
    unit/test_synth_lib.cpp
)
target_link_libraries(test_synth_lib
    PRIVATE
        curv_tools_lib
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_synth_lib)
add_test(NAME test_synth_lib COMMAND test_synth_lib)

# ------------------------------------------------------------------------------
# Unit Test: test_benchmark_lib (Phase B throughput benchmark harness, TDD)
# ------------------------------------------------------------------------------
add_executable(test_benchmark_lib
    unit/test_benchmark_lib.cpp
)
target_link_libraries(test_benchmark_lib
    PRIVATE
        curv_tools_lib
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_benchmark_lib)
add_test(NAME test_benchmark_lib COMMAND test_benchmark_lib)

# ------------------------------------------------------------------------------
# Unit Test: test_cli_lib (perception CLI runner, TDD)
# ------------------------------------------------------------------------------
add_executable(test_cli_lib
    unit/test_cli_lib.cpp
)
target_link_libraries(test_cli_lib
    PRIVATE
        curv_tools_lib
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_cli_lib)
add_test(NAME test_cli_lib COMMAND test_cli_lib)

# ------------------------------------------------------------------------------
# Unit Test: test_hand_inspector_lib (hand inspector CLI runner, TDD)
# ------------------------------------------------------------------------------
add_executable(test_hand_inspector_lib
    unit/test_hand_inspector_lib.cpp
)
target_link_libraries(test_hand_inspector_lib
    PRIVATE
        curv_tools_lib
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_hand_inspector_lib)
add_test(NAME test_hand_inspector_lib COMMAND test_hand_inspector_lib)

# ------------------------------------------------------------------------------
# Unit Test: test_demo_lib (desktop demo runner, TDD)
# ------------------------------------------------------------------------------
add_executable(test_demo_lib
    unit/test_demo_lib.cpp
)
target_link_libraries(test_demo_lib
    PRIVATE
        curv_tools_lib
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_demo_lib)
add_test(NAME test_demo_lib COMMAND test_demo_lib)

# ------------------------------------------------------------------------------
# Unit Test: test_parameter_resolver (dynamic Steger parameter scaling, TDD)
# ------------------------------------------------------------------------------
add_executable(test_parameter_resolver
    unit/test_parameter_resolver.cpp
)
target_link_libraries(test_parameter_resolver
    PRIVATE
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_parameter_resolver)
add_test(NAME test_parameter_resolver COMMAND test_parameter_resolver)

# ------------------------------------------------------------------------------
# Unit Test: test_cctv_adapter (CCTV frame ingestion & quality validation, TDD)
# ------------------------------------------------------------------------------
add_executable(test_cctv_adapter
    unit/test_cctv_adapter.cpp
)
target_compile_definitions(test_cctv_adapter
    PRIVATE
        CURV_PROJECT_ROOT="${CMAKE_SOURCE_DIR}"
)
target_link_libraries(test_cctv_adapter
    PRIVATE
        curv_core
        curv_adapters
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_cctv_adapter)
add_test(NAME test_cctv_adapter
    COMMAND test_cctv_adapter
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
)

# ------------------------------------------------------------------------------
# Unit Test: test_cctv_stream_adapter (Stage 2 ring-buffer / recorded video)
# ------------------------------------------------------------------------------
add_executable(test_cctv_stream_adapter
    unit/test_cctv_stream_adapter.cpp
)
target_compile_definitions(test_cctv_stream_adapter
    PRIVATE
        CURV_PROJECT_ROOT="${CMAKE_SOURCE_DIR}"
)
target_link_libraries(test_cctv_stream_adapter
    PRIVATE
        curv_core
        curv_adapters
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_cctv_stream_adapter)
add_test(NAME test_cctv_stream_adapter
    COMMAND test_cctv_stream_adapter
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
)

# ------------------------------------------------------------------------------
# Unit Test: test_cctv_video_source (Phase 2 recorded video + FPS stride)
# ------------------------------------------------------------------------------
add_executable(test_cctv_video_source
    unit/test_cctv_video_source.cpp
)
target_compile_definitions(test_cctv_video_source
    PRIVATE
        CURV_PROJECT_ROOT="${CMAKE_SOURCE_DIR}"
)
target_link_libraries(test_cctv_video_source
    PRIVATE
        curv_core
        curv_adapters
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_cctv_video_source)
add_test(NAME test_cctv_video_source
    COMMAND test_cctv_video_source
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
)

# ------------------------------------------------------------------------------
# Unit Test: test_cctv_http_snapshot (Phase 3 HTTP / mock / cache)
# ------------------------------------------------------------------------------
add_executable(test_cctv_http_snapshot
    unit/test_cctv_http_snapshot.cpp
)
target_link_libraries(test_cctv_http_snapshot
    PRIVATE
        curv_core
        curv_adapters
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_cctv_http_snapshot)
add_test(NAME test_cctv_http_snapshot
    COMMAND test_cctv_http_snapshot
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
)

# ------------------------------------------------------------------------------
# Unit Test: test_hand_anatomy_decomposer (HandAnatomyDecomposer, TDD)
# ------------------------------------------------------------------------------
add_executable(test_hand_anatomy_decomposer
    unit/test_hand_anatomy_decomposer.cpp
)
target_link_libraries(test_hand_anatomy_decomposer
    PRIVATE
        curv_domains
        curv_adapters
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_hand_anatomy_decomposer)
add_test(NAME test_hand_anatomy_decomposer COMMAND test_hand_anatomy_decomposer)

# ------------------------------------------------------------------------------
# Unit Test: test_crease_chain_linker (CreaseChainLinker, TDD)
# ------------------------------------------------------------------------------
add_executable(test_crease_chain_linker
    unit/test_crease_chain_linker.cpp
)
target_link_libraries(test_crease_chain_linker
    PRIVATE
        curv_domains
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_crease_chain_linker)
add_test(NAME test_crease_chain_linker COMMAND test_crease_chain_linker)

# ------------------------------------------------------------------------------
# Unit Test: test_crease_geometry (CreaseGeometry, TDD)
# ------------------------------------------------------------------------------
add_executable(test_crease_geometry
    unit/test_crease_geometry.cpp
)
target_link_libraries(test_crease_geometry
    PRIVATE
        curv_domains
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_crease_geometry)
add_test(NAME test_crease_geometry COMMAND test_crease_geometry)

# ------------------------------------------------------------------------------
# Unit Test: test_fate_line_analyzer (FateLineAnalyzer, TDD)
# ------------------------------------------------------------------------------
add_executable(test_fate_line_analyzer
    unit/test_fate_line_analyzer.cpp
)
target_link_libraries(test_fate_line_analyzer
    PRIVATE
        curv_domains
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_fate_line_analyzer)
add_test(NAME test_fate_line_analyzer COMMAND test_fate_line_analyzer)

# ------------------------------------------------------------------------------
# Unit Test: test_floor_plan_domain (FloorPlanDomainPack, TDD)
# ------------------------------------------------------------------------------
add_executable(test_floor_plan_domain
    unit/test_floor_plan_domain.cpp
)
target_link_libraries(test_floor_plan_domain
    PRIVATE
        curv_domains
        curv_capi
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_floor_plan_domain)
add_test(NAME test_floor_plan_domain COMMAND test_floor_plan_domain)

# ------------------------------------------------------------------------------
# Unit Test: test_curve_chunker (CurveChunker, TDD)
# ------------------------------------------------------------------------------
add_executable(test_curve_chunker
    unit/test_curve_chunker.cpp
)
target_link_libraries(test_curve_chunker
    PRIVATE
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_curve_chunker)
add_test(NAME test_curve_chunker COMMAND test_curve_chunker)

# ------------------------------------------------------------------------------
# Unit Test: test_micro_pattern_detector (MicroPatternDetector, TDD)
# ------------------------------------------------------------------------------
add_executable(test_micro_pattern_detector
    unit/test_micro_pattern_detector.cpp
)
target_link_libraries(test_micro_pattern_detector
    PRIVATE
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_micro_pattern_detector)
add_test(NAME test_micro_pattern_detector COMMAND test_micro_pattern_detector)



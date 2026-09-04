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
        curv_core
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
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_surface_inspection)
add_test(NAME test_surface_inspection COMMAND test_surface_inspection)

# ------------------------------------------------------------------------------
# Unit Test: test_overlay_renderer
# ------------------------------------------------------------------------------
add_executable(test_overlay_renderer
    unit/test_overlay_renderer.cpp
)
target_link_libraries(test_overlay_renderer
    PRIVATE
        curv_adapters
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_overlay_renderer)
add_test(NAME test_overlay_renderer COMMAND test_overlay_renderer)

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
        curv_core
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
        curv_core
        GTest::gtest_main
        ${OpenCV_LIBS}
)
vision_set_compiler_warnings(test_vlm_formatter)
add_test(NAME test_vlm_formatter COMMAND test_vlm_formatter)



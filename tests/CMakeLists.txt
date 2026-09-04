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

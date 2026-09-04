# ==============================================================================
# vision-perception - Reusable Computer Vision Core Monorepo
# ==============================================================================
cmake_minimum_required(VERSION 3.24)

project(vision-perception
    VERSION 0.1.0
    DESCRIPTION "Reusable domain-agnostic computer vision perception core"
    LANGUAGES CXX
)

# ------------------------------------------------------------------------------
# C++ Standard & Compiler Configurations
# ------------------------------------------------------------------------------
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Export compile commands for clangd/IDE integration
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Default to Release if CMAKE_BUILD_TYPE is unset
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Choose the type of build." FORCE)
endif()

# Module path and system prefix paths
list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
list(APPEND CMAKE_PREFIX_PATH
    "/opt/homebrew"
    "/opt/homebrew/opt/opencv"
    "/opt/homebrew/opt/spdlog"
    "/opt/homebrew/opt/nlohmann-json"
    "/opt/homebrew/opt/googletest"
    "/usr/local"
    "/usr/local/opt/opencv"
)

# Include strict compiler warnings module
include(cmake/compiler_warnings.cmake)

# ------------------------------------------------------------------------------
# Dependencies (nlohmann_json & spdlog)
# ------------------------------------------------------------------------------
include(FetchContent)

# 1. nlohmann_json
find_package(nlohmann_json QUIET)
if(NOT nlohmann_json_FOUND)
    message(STATUS "[vision-perception] Fetching nlohmann_json via FetchContent...")
    FetchContent_Declare(
        nlohmann_json
        GIT_REPOSITORY https://github.com/nlohmann/json.git
        GIT_TAG        v3.11.3
    )
    set(JSON_BuildTests OFF CACHE INTERNAL "")
    set(JSON_Install OFF CACHE INTERNAL "")
    FetchContent_MakeAvailable(nlohmann_json)
else()
    message(STATUS "[vision-perception] Found local nlohmann_json: ${nlohmann_json_DIR}")
endif()

# 2. spdlog
find_package(spdlog QUIET)
if(NOT spdlog_FOUND)
    message(STATUS "[vision-perception] Fetching spdlog via FetchContent...")
    set(SPDLOG_FMT_EXTERNAL ON CACHE INTERNAL "")
    set(SPDLOG_BUILD_TESTS OFF CACHE INTERNAL "")
    set(SPDLOG_BUILD_EXAMPLE OFF CACHE INTERNAL "")
    FetchContent_Declare(
        spdlog
        GIT_REPOSITORY https://github.com/gabime/spdlog.git
        GIT_TAG        v1.15.0
    )
    FetchContent_MakeAvailable(spdlog)
else()
    message(STATUS "[vision-perception] Found local spdlog: ${spdlog_DIR}")
endif()

# ------------------------------------------------------------------------------
# Dependency: OpenCV
# ------------------------------------------------------------------------------
find_package(OpenCV REQUIRED COMPONENTS core imgproc imgcodecs)

# ------------------------------------------------------------------------------
# Diagnostic Status Messages
# ------------------------------------------------------------------------------
message(STATUS "==================================================")
message(STATUS "vision-perception Build Configuration Status:")
message(STATUS "  - Compiler:    ${CMAKE_CXX_COMPILER_ID} (Version ${CMAKE_CXX_COMPILER_VERSION})")
message(STATUS "  - Build Type:  ${CMAKE_BUILD_TYPE}")
message(STATUS "  - C++ Standard: C++${CMAKE_CXX_STANDARD}")
message(STATUS "  - OpenCV:      Version ${OpenCV_VERSION}")
message(STATUS "  - OpenCV Inc:  ${OpenCV_INCLUDE_DIRS}")
message(STATUS "==================================================")

# ------------------------------------------------------------------------------
# Subdirectories & Testing
# ------------------------------------------------------------------------------
enable_testing()

add_subdirectory(core)
add_subdirectory(adapters)
add_subdirectory(domains)
add_subdirectory(apps/desktop_pipeline_demo)
add_subdirectory(tools)
add_subdirectory(tests)

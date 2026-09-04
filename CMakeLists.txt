# ==============================================================================
# vision-perception - Reusable Computer Vision Core Monorepo
# ==============================================================================
cmake_minimum_required(VERSION 3.20)

project(vision-perception
    VERSION 0.1.0
    DESCRIPTION "Reusable domain-agnostic computer vision perception core"
    LANGUAGES CXX C
)

# ------------------------------------------------------------------------------
# Global Settings & Standards
# ------------------------------------------------------------------------------
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Export compile commands for clangd/IDE integration
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Module path
list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

# ------------------------------------------------------------------------------
# Build Options
# ------------------------------------------------------------------------------
option(BUILD_TESTING "Build unit tests" ON)
option(BUILD_APPS "Build sample application executables" OFF)
option(BUILD_TOOLS "Build developer tools" OFF)

# ------------------------------------------------------------------------------
# Include Modules & Dependencies
# ------------------------------------------------------------------------------
include(cmake/compiler_warnings.cmake)
include(cmake/dependencies.cmake)

# ------------------------------------------------------------------------------
# Subdirectories
# ------------------------------------------------------------------------------
add_subdirectory(core)

if(BUILD_TESTING)
    enable_testing()
    add_subdirectory(core/tests)
endif()

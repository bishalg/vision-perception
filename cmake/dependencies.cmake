# ==============================================================================
# Dependency Management Module
# ==============================================================================

include(FetchContent)

# ------------------------------------------------------------------------------
# 1. OpenCV Discovery
# ------------------------------------------------------------------------------
# Allow hint paths for macOS Homebrew / Linux / Windows
list(APPEND CMAKE_PREFIX_PATH
    "/opt/homebrew/opt/opencv"
    "/usr/local/opt/opencv"
)

find_package(OpenCV QUIET COMPONENTS core imgproc calib3d)

if(OpenCV_FOUND)
    message(STATUS "[vision-perception] Found OpenCV version: ${OpenCV_VERSION}")
    message(STATUS "[vision-perception] OpenCV include directories: ${OpenCV_INCLUDE_DIRS}")
    message(STATUS "[vision-perception] OpenCV libraries: ${OpenCV_LIBS}")
else()
    message(STATUS "[vision-perception] OpenCV not found on host system. External adapter builds requiring OpenCV will be deferred.")
endif()

# ------------------------------------------------------------------------------
# 2. GoogleTest (for unit testing)
# ------------------------------------------------------------------------------
if(BUILD_TESTING)
    find_package(GTest QUIET)
    if(NOT GTest_FOUND)
        message(STATUS "[vision-perception] GTest not found locally. Fetching GoogleTest via FetchContent...")
        FetchContent_Declare(
            googletest
            GIT_REPOSITORY https://github.com/google/googletest.git
            GIT_TAG        v1.14.0
        )
        # Prevent GoogleTest from overriding parent compile/runtime options on Windows
        set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
        set(INSTALL_GTEST OFF CACHE BOOL "" FORCE)
        set(BUILD_GMOCK OFF CACHE BOOL "" FORCE)
        FetchContent_MakeAvailable(googletest)
    endif()
endif()

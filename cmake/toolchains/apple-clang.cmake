# ==============================================================================
# Apple Clang CMake Toolchain
# ==============================================================================
set(CMAKE_SYSTEM_NAME Darwin)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

# Modern standard flags
set(CMAKE_CXX_STANDARD 20 CACHE STRING "C++ Standard")
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

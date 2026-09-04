# ==============================================================================
# Android NDK CMake Toolchain Reference
# ==============================================================================
# Expected usage:
# cmake -DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake \
#       -DANDROID_ABI=arm64-v8a \
#       -DANDROID_PLATFORM=android-26 \
#       -DANDROID_STL=c++_static ..
set(ANDROID_STL c++_static)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

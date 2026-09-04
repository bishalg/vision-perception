# ==============================================================================
# curv_adapters Static Library Target
# ==============================================================================

add_library(curv_adapters STATIC
    opencv/src/OverlayRenderer.cpp
)

target_include_directories(curv_adapters
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/opencv/include>
        $<INSTALL_INTERFACE:include>
)

target_link_libraries(curv_adapters
    PUBLIC
        curv_core
        ${OpenCV_LIBS}
)

vision_set_compiler_warnings(curv_adapters)

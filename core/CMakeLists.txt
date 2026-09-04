# ==============================================================================
# vision_core Library Target
# ==============================================================================

add_library(vision_core
    src/vision_core.cpp
)

target_include_directories(vision_core
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
)

if(TARGET OpenCV::opencv_core)
    target_link_libraries(vision_core PUBLIC OpenCV::opencv_core)
elseif(OpenCV_FOUND)
    target_include_directories(vision_core PUBLIC ${OpenCV_INCLUDE_DIRS})
    target_link_libraries(vision_core PUBLIC ${OpenCV_LIBS})
endif()

vision_set_compiler_warnings(vision_core)

# ==============================================================================
# curv_adapters Static Library Target
# ==============================================================================

add_library(curv_adapters STATIC
    opencv/src/OverlayRenderer.cpp
)

target_include_directories(curv_adapters
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/opencv/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/capi/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/c_api/include>
        $<INSTALL_INTERFACE:include>
)

target_link_libraries(curv_adapters
    PUBLIC
        curv_core
        ${OpenCV_LIBS}
)

vision_set_compiler_warnings(curv_adapters)

# ==============================================================================
# curv_capi Shared / Static Library Target (Cross-Platform C-ABI)
# ==============================================================================

add_library(curv_capi SHARED
    capi/src/curv_capi.cpp
)

target_compile_definitions(curv_capi
    PRIVATE
        CURV_CAPI_EXPORTS
)

target_include_directories(curv_capi
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/capi/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/c_api/include>
        $<INSTALL_INTERFACE:include>
)

target_link_libraries(curv_capi
    PRIVATE
        curv_core
        curv_domains
        curv_adapters
        ${OpenCV_LIBS}
        nlohmann_json::nlohmann_json
)

vision_set_compiler_warnings(curv_capi)

add_library(curv_api ALIAS curv_capi)

# Static variant for foreign embedding
add_library(curv_capi_static STATIC
    capi/src/curv_capi.cpp
)

target_include_directories(curv_capi_static
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/capi/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/c_api/include>
        $<INSTALL_INTERFACE:include>
)

target_link_libraries(curv_capi_static
    PUBLIC
        curv_core
        curv_domains
        curv_adapters
        ${OpenCV_LIBS}
        nlohmann_json::nlohmann_json
)

vision_set_compiler_warnings(curv_capi_static)

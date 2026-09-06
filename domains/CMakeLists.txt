# ==============================================================================
# curv_domains / curv_domain_palm Static Library Target
# ==============================================================================

add_library(curv_domains STATIC
    surface_inspection/src/SurfaceInspectionPack.cpp
    palm/src/PalmDomainPack.cpp
    palm/src/PalmZones.cpp
    palm/src/VlmPromptFormatter.cpp
    geospatial/src/GeospatialFloodPack.cpp
)

add_library(curv_domain_palm ALIAS curv_domains)

target_include_directories(curv_domains
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/surface_inspection/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/palm/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/geospatial/include>
        $<INSTALL_INTERFACE:include>
)


target_link_libraries(curv_domains
    PUBLIC
        curv_core
)

vision_set_compiler_warnings(curv_domains)

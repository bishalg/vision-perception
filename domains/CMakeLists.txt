# ==============================================================================
# curv_domains / curv_domain_palm Static Library Target
# ==============================================================================

add_library(curv_domains STATIC
    surface_inspection/src/SurfaceInspectionPack.cpp
    palm/src/PalmDomainPack.cpp
    palm/src/PalmZones.cpp
    palm/src/HandAnatomyDecomposer.cpp
    palm/src/CreaseChainLinker.cpp
    palm/src/CreaseGeometry.cpp
    palm/src/FateLineAnalyzer.cpp
    palm/src/VlmPromptFormatter.cpp
    geospatial/src/GeospatialFloodPack.cpp
    floor_plan/src/FloorPlanDomainPack.cpp
    floor_plan/src/WallSegmentExtractor.cpp
    floor_plan/src/WallJunctionGraph.cpp
    floor_plan/src/RoomPolygonDetector.cpp
    floor_plan/src/VastuMandalaOverlay.cpp
)

add_library(curv_domain_palm ALIAS curv_domains)
add_library(curv_domain_floor_plan ALIAS curv_domains)

target_include_directories(curv_domains
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/surface_inspection/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/palm/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/geospatial/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/floor_plan/include>
        $<INSTALL_INTERFACE:include>
)


target_link_libraries(curv_domains
    PUBLIC
        curv_core
)

vision_set_compiler_warnings(curv_domains)

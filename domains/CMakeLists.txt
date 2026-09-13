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
    palm/src/LifeLineAnalyzer.cpp
    palm/src/HeartHeadAnalyzer.cpp
    palm/src/MountMarkDetector.cpp
    palm/src/VlmPromptFormatter.cpp
    geospatial/src/GeospatialFloodPack.cpp
    floor_plan/src/FloorPlanDomainPack.cpp
    floor_plan/src/WallSegmentExtractor.cpp
    floor_plan/src/WallJunctionGraph.cpp
    floor_plan/src/RoomPolygonDetector.cpp
    floor_plan/src/VastuMandalaOverlay.cpp
    retina/src/RetinaDomainPack.cpp
    driver_monitoring/src/DriverGazePack.cpp
    asd_screening/src/AsdBiomarkerPack.cpp
)

add_library(curv_domain_palm ALIAS curv_domains)
add_library(curv_domain_floor_plan ALIAS curv_domains)
add_library(curv_domain_retina ALIAS curv_domains)
add_library(curv_domain_driver_monitoring ALIAS curv_domains)
add_library(curv_domain_asd_screening ALIAS curv_domains)

target_include_directories(curv_domains
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/surface_inspection/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/palm/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/geospatial/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/floor_plan/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/retina/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/driver_monitoring/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/asd_screening/include>
        $<INSTALL_INTERFACE:include>
)


target_link_libraries(curv_domains
    PUBLIC
        curv_core
)

vision_set_compiler_warnings(curv_domains)

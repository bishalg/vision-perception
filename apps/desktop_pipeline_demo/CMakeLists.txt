# ==============================================================================
# Desktop Pipeline Demo Application Target
# ==============================================================================

add_executable(desktop_pipeline_demo
    main.cpp
)

target_link_libraries(desktop_pipeline_demo
    PRIVATE
        vision_perception_core
)

vision_set_compiler_warnings(desktop_pipeline_demo)

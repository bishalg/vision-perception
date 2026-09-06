# ==============================================================================
# Desktop Pipeline Demo Application Target
# ==============================================================================

add_executable(desktop_pipeline_demo
    main.cpp
)

target_link_libraries(desktop_pipeline_demo
    PRIVATE
        curv_tools_lib
)

vision_set_compiler_warnings(desktop_pipeline_demo)

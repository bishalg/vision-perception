# ==============================================================================
# curv_cli Verification Tool Target
# ==============================================================================

add_executable(curv_cli
    curv_cli.cpp
)

target_link_libraries(curv_cli
    PRIVATE
        curv_core
        curv_adapters
        curv_domains
        ${OpenCV_LIBS}
)

vision_set_compiler_warnings(curv_cli)

# ==============================================================================
# hand_inspector_cli Target
# ==============================================================================

add_executable(hand_inspector_cli
    hand_inspector_cli.cpp
)

target_link_libraries(hand_inspector_cli
    PRIVATE
        curv_core
        curv_adapters
        curv_domains
        ${OpenCV_LIBS}
)

vision_set_compiler_warnings(hand_inspector_cli)

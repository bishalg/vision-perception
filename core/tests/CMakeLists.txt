# ==============================================================================
# core/tests CMakeLists.txt
# ==============================================================================

add_executable(test_geometry_types
    test_geometry_types.cpp
)

target_link_libraries(test_geometry_types
    PRIVATE
        vision_core
)

vision_set_compiler_warnings(test_geometry_types)

add_test(NAME test_geometry_types COMMAND test_geometry_types)

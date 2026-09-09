# ==============================================================================
# curv_tools_lib - Testable support libraries for the CLI tools
# ==============================================================================

add_library(curv_tools_lib STATIC
    lib/curv/tools/synth_lib.cpp
    lib/curv/tools/evaluate_lib.cpp
    lib/curv/tools/benchmark_lib.cpp
    lib/curv/tools/cli_lib.cpp
    lib/curv/tools/hand_inspector_lib.cpp
    lib/curv/tools/demo_lib.cpp
    lib/curv/tools/video_pipeline_demo_lib.cpp
)

target_include_directories(curv_tools_lib
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/lib>
)

target_link_libraries(curv_tools_lib
    PUBLIC
        curv_core
        curv_domains
        curv_adapters
        ${OpenCV_LIBS}
        nlohmann_json::nlohmann_json
)

vision_set_compiler_warnings(curv_tools_lib)

# ==============================================================================
# Tool executables (thin mains; all logic lives in curv_tools_lib)
# ==============================================================================

add_executable(curv_cli
    curv_cli.cpp
)

target_link_libraries(curv_cli
    PRIVATE
        curv_tools_lib
        ${OpenCV_LIBS}
)

vision_set_compiler_warnings(curv_cli)

add_executable(hand_inspector_cli
    hand_inspector_cli.cpp
)

target_link_libraries(hand_inspector_cli
    PRIVATE
        curv_tools_lib
        ${OpenCV_LIBS}
)

vision_set_compiler_warnings(hand_inspector_cli)

add_executable(synth_generator
    synth_generator.cpp
)

target_link_libraries(synth_generator
    PRIVATE
        curv_tools_lib
)

vision_set_compiler_warnings(synth_generator)

add_executable(evaluate
    evaluate.cpp
)

target_link_libraries(evaluate
    PRIVATE
        curv_tools_lib
)

vision_set_compiler_warnings(evaluate)

add_executable(benchmark
    benchmark.cpp
)

target_link_libraries(benchmark
    PRIVATE
        curv_tools_lib
)

vision_set_compiler_warnings(benchmark)

add_executable(video_pipeline_demo
    video_pipeline_demo.cpp
)

target_link_libraries(video_pipeline_demo
    PRIVATE
        curv_tools_lib
        ${OpenCV_LIBS}
)

vision_set_compiler_warnings(video_pipeline_demo)

# ==============================================================================
# Roadmap M6 accuracy gate: synth_generator -> curv_cli -> evaluate (RMSE < 0.1px)
# Registered as a CTest test driven by tools/eval_gate.cmake.
# ==============================================================================

add_test(NAME evaluation_rmse_gate
    COMMAND ${CMAKE_COMMAND}
        -DSYNTH_GENERATOR=$<TARGET_FILE:synth_generator>
        -DCURV_CLI=$<TARGET_FILE:curv_cli>
        -DEVALUATE=$<TARGET_FILE:evaluate>
        -DGATE_PX=0.1
        -P ${CMAKE_CURRENT_SOURCE_DIR}/eval_gate.cmake
)

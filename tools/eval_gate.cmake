# ==============================================================================
# eval_gate.cmake - CTest script for the roadmap M6 accuracy gate.
#
# Pipeline: synth_generator (line, with known ground truth) -> curv_cli
# (extraction to evidence JSON) -> evaluate (RMSE vs ground truth).
# Fails (non-zero) when the extraction RMSE is >= GATE_PX (0.1 px roadmap gate).
#
# Inputs (passed by tools/CMakeLists.txt):
#   -DSYNTH_GENERATOR=<path> -DCURV_CLI=<path> -DEVALUATE=<path> -DGATE_PX=<px>
# ==============================================================================

if(NOT SYNTH_GENERATOR OR NOT CURV_CLI OR NOT EVALUATE)
    message(FATAL_ERROR "eval_gate.cmake requires SYNTH_GENERATOR, CURV_CLI and EVALUATE paths")
endif()

set(GATE_PX "0.1" CACHE STRING "RMSE gate in pixels")
set(WORK_DIR "${CMAKE_BINARY_DIR}/eval_gate_tmp")
file(REMOVE_RECURSE "${WORK_DIR}")
file(MAKE_DIRECTORY "${WORK_DIR}")

set(STATUS "")
function(run_step DESCRIPTION)
    # ARGN = the command and its arguments (everything after DESCRIPTION).
    execute_process(COMMAND ${ARGN}
        WORKING_DIRECTORY "${WORK_DIR}"
        RESULT_VARIABLE RESULT
        OUTPUT_VARIABLE STDOUT
        ERROR_VARIABLE STDERR)
    if(NOT RESULT EQUAL 0)
        message(FATAL_ERROR "${DESCRIPTION} failed (exit ${RESULT}):\n${STDOUT}\n${STDERR}")
    endif()
    set(STATUS "${STATUS}${DESCRIPTION}: ok\n" PARENT_SCOPE)
endfunction()

# 1. Synthetic image with analytical ground truth (noise-free, bright ridges).
# GT sampling (0.05 px) must be dense relative to the 0.1 px accuracy gate:
# nearest-sample quantization contributes at most 0.025 px of the residual.
run_step("synth_generator" "${SYNTH_GENERATOR}"
    --type line --width 512 --height 512 --sample-step 0.05 --out-dir "${WORK_DIR}")

# 2. Run the full perception pipeline to produce evidence JSON.
run_step("curv_cli" "${CURV_CLI}"
    --image "${WORK_DIR}/line_image.png" --json "${WORK_DIR}/evidence.json"
    --out "${WORK_DIR}/overlay.png" --sigma 1.5 --low 0.5 --high 1.5)

# 3. Evaluate extraction accuracy against the ground truth. The evaluator's
# exit code IS the gate: 0 = within gate, 2 = RMSE >= GATE_PX or misses.
# --end-margin 10 trims the rasterized line-cap / ridge run-out band around
# the GT curve endpoints, which does not represent curve-body localization.
run_step("evaluate" "${EVALUATE}"
    --truth "${WORK_DIR}/line_truth.json" --evidence "${WORK_DIR}/evidence.json"
    --gate "${GATE_PX}" --end-margin 10 --report "${WORK_DIR}/eval_report.json")

file(REMOVE_RECURSE "${WORK_DIR}")
message(STATUS "evaluation_rmse_gate PASSED (RMSE < ${GATE_PX} px)")

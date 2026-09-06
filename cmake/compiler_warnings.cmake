# ==============================================================================
# Compiler Warnings and Strictness Configuration
# ==============================================================================

function(vision_set_compiler_warnings target_name)
    if(NOT TARGET ${target_name})
        message(FATAL_ERROR "vision_set_compiler_warnings called on non-target: ${target_name}")
    endif()

    set(CLANG_GCC_WARNINGS
        -Wall
        -Wextra
        -Wpedantic
        -Wconversion
        -Wsign-conversion
        -Wcast-align
        -Wunused
        -Wshadow
        -Wnon-virtual-dtor
        -Woverloaded-virtual
        -Wnull-dereference
        -Wformat=2
    )

    set(MSVC_WARNINGS
        /W4
        /permissive-
        /w14242
        /w14254
        /w14263
        /w14265
        /w14287
        /we4289
        /w14296
        /w14311
        /w14545
        /w14546
        /w14547
        /w14549
        /w14555
        /w14619
        /w14640
        /w14826
        /w14905
        /w14906
        /w14928
    )

    if(VISION_WERROR)
        list(APPEND CLANG_GCC_WARNINGS -Werror)
        list(APPEND MSVC_WARNINGS /WX)
    endif()

    if(CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
        # GCC 13 at -O3 false-positives -Wnull-dereference from inside libstdc++
        # headers (stl_algobase std::copy, <streambuf> xsputn/xsgetn) once they
        # are inlined into user code. Clang keeps the warning; GCC drops it.
        list(APPEND CLANG_GCC_WARNINGS -Wno-null-dereference)
    endif()

    if(MSVC)
        target_compile_options(${target_name} PRIVATE ${MSVC_WARNINGS})
    elseif(CMAKE_CXX_COMPILER_ID MATCHES ".*Clang" OR CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
        target_compile_options(${target_name} PRIVATE ${CLANG_GCC_WARNINGS})
    endif()
endfunction()

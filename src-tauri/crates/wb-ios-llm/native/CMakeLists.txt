# SPDX-License-Identifier: GPL-3.0-or-later
#
# llama.cpp b11624 as one static library with a small C API (wb_llm.h): the
# server's own request code (tools/server "server-context", the part
# llama-server and llama-cli share) without its HTTP listener, router, web UI
# or subprocess support. native/build.sh drives this file and then hides every
# symbol except the wb_llm_* API.
cmake_minimum_required(VERSION 3.21)
project(wb_llm C CXX)

if (NOT WB_LLAMA_SRC)
    message(FATAL_ERROR "WB_LLAMA_SRC (the llama.cpp b11624 source tree) is required")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# The same choices as ci/llama-server/build.sh (the Mac sidecar), minus the
# executables.
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(GGML_METAL_EMBED_LIBRARY ON CACHE BOOL "" FORCE)
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
set(GGML_RPC OFF CACHE BOOL "" FORCE)
set(GGML_BLAS OFF CACHE BOOL "" FORCE)
set(GGML_BACKEND_DL OFF CACHE BOOL "" FORCE)
set(LLAMA_OPENSSL OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_BORINGSSL OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_UI OFF CACHE BOOL "" FORCE)
set(LLAMA_USE_PREBUILT_UI OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TOOLS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_SERVER OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_APP OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_COMMON ON CACHE BOOL "" FORCE)
set(LLAMA_BUILD_MTMD ON CACHE BOOL "" FORCE)
set(LLAMA_SUBPROCESS OFF CACHE BOOL "" FORCE)
set(LLAMA_TOOLS_INSTALL OFF CACHE BOOL "" FORCE)

add_subdirectory(${WB_LLAMA_SRC} llama EXCLUDE_FROM_ALL)

set(SERVER ${WB_LLAMA_SRC}/tools/server)
add_library(wb_llm STATIC
    wb_llm.cpp
    ${SERVER}/server-chat.cpp
    ${SERVER}/server-task.cpp
    ${SERVER}/server-queue.cpp
    ${SERVER}/server-common.cpp
    ${SERVER}/server-decision.cpp
    ${SERVER}/server-context.cpp
    ${SERVER}/server-stream.cpp
    ${SERVER}/server-tools.cpp
    ${SERVER}/server-mcp.cpp
    ${SERVER}/server-schema.cpp
)
target_include_directories(wb_llm PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${SERVER}
    ${WB_LLAMA_SRC}
    ${WB_LLAMA_SRC}/tools/mtmd
)
target_link_libraries(wb_llm PUBLIC llama-common mtmd)

# ggml-metal's one Objective-C class. transcribe-cpp's ggml has a class of the
# same name; Objective-C class names are process-wide, so ours is renamed.
if (TARGET ggml-metal)
    target_compile_definitions(ggml-metal PRIVATE GGMLMetalClass=WBLLMGGMLMetalClass)
endif()

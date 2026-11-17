#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds llama.cpp b11624 (the tag and source hash of the Mac sidecar,
# ci/llama-server/build.sh) plus wb_llm.cpp into ONE static library for a
# Rust target triple, with every symbol hidden except the wb_llm_* API
# (exports.txt). Hiding is what lets it share a binary with transcribe-cpp,
# which carries its own ggml under the same symbol names.
#
# Usage: native/build.sh <aarch64-apple-darwin|aarch64-apple-ios|aarch64-apple-ios-sim>
# Prints the directory holding libwb_ios_llm.a on the last line.
#
# The build is cached per triple (not per cargo profile) under
#   ${WB_IOS_LLM_CACHE:-<crate>/target/native}/<triple>
# and redone only when the pin, the flags or a file in native/ changes.
#
# Env:
#   WB_LLAMA_SRC_TARBALL  local copy of the source tarball (sha256-checked)
#   WB_IOS_LLM_CACHE      cache root
#   WB_LLAMA_JOBS         parallel build jobs (default: logical CPUs)
set -euo pipefail

TAG="b11624"
COMMIT="af0579c78a07a555d84830d01da7b61cfadbc7ef"
SRC_URL="https://github.com/ggml-org/llama.cpp/archive/refs/tags/${TAG}.tar.gz"
SRC_SHA256="9a44be53ab3c76ab958131e8d9f2e14ab1e885cf1841afdcd77ca936c066249f"
# The M1 baseline of the Mac sidecar. On iPhone this code runs only where the
# cleanup model is loaded (8 GB and more: A17 Pro and later, all newer).
CPU_ARCH="armv8.4-a+fp16+dotprod"

TRIPLE="${1:?usage: build.sh <target triple>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE="$(cd "${HERE}/.." && pwd)"
CACHE="${WB_IOS_LLM_CACHE:-${CRATE}/target/native}"
OUT="${CACHE}/${TRIPLE}"
SRC="${CACHE}/src-${TAG}"
BUILD="${OUT}/build"
LIB="${OUT}/libwb_ios_llm.a"

case "${TRIPLE}" in
  aarch64-apple-darwin)
    SDK="macosx"; MIN="13.0"; LD_PLATFORM="macos"; PLATFORM=(-DCMAKE_SYSTEM_NAME=Darwin)
    ;;
  aarch64-apple-ios)
    SDK="iphoneos"; MIN="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"; LD_PLATFORM="ios"
    PLATFORM=(-DCMAKE_SYSTEM_NAME=iOS)
    ;;
  aarch64-apple-ios-sim)
    SDK="iphonesimulator"; MIN="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"; LD_PLATFORM="ios-simulator"
    PLATFORM=(-DCMAKE_SYSTEM_NAME=iOS)
    ;;
  *)
    echo "build.sh: unsupported target ${TRIPLE}" >&2
    exit 1
    ;;
esac
SYSROOT="$(xcrun --sdk "${SDK}" --show-sdk-path)"

CMAKE_FLAGS=(
  -DCMAKE_BUILD_TYPE=Release
  "${PLATFORM[@]}"
  -DCMAKE_OSX_SYSROOT="${SYSROOT}"
  -DCMAKE_OSX_DEPLOYMENT_TARGET="${MIN}"
  -DCMAKE_OSX_ARCHITECTURES=arm64
  -DGGML_METAL=ON
  -DGGML_CPU_ARM_ARCH="${CPU_ARCH}"
  -DLLAMA_BUILD_NUMBER="${TAG#b}"
  -DLLAMA_BUILD_COMMIT="${COMMIT:0:9}"
  # Tentative definitions (ggml's lookup tables) as ordinary data: `ld -r`
  # cannot hide a common symbol.
  -DCMAKE_C_FLAGS=-fno-common
  -DCMAKE_C_VISIBILITY_PRESET=hidden
  -DCMAKE_CXX_VISIBILITY_PRESET=hidden
  -DCMAKE_VISIBILITY_INLINES_HIDDEN=ON
)

sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }
stamp_text() {
  printf 'tag=%s\nsrc_sha256=%s\nflags=%s\nnative=%s\n' "${TAG}" "${SRC_SHA256}" \
    "${CMAKE_FLAGS[*]}" \
    "$(cat "${HERE}/CMakeLists.txt" "${HERE}/wb_llm.cpp" "${HERE}/wb_llm.h" \
      "${HERE}/exports.txt" "${HERE}/build.sh" | shasum -a 256 | awk '{print $1}')"
}
if [[ -f "${LIB}" && -f "${OUT}/stamp" && "$(cat "${OUT}/stamp")" == "$(stamp_text)" ]]; then
  echo "${OUT}"
  exit 0
fi

command -v cmake >/dev/null || { echo "build.sh: cmake is required (brew install cmake)" >&2; exit 1; }
mkdir -p "${CACHE}" "${OUT}"

# --- source (shared by every triple) ----------------------------------------
if [[ ! -f "${SRC}/CMakeLists.txt" ]]; then
  TARBALL="${CACHE}/llama.cpp-${TAG}.tar.gz"
  for candidate in "${WB_LLAMA_SRC_TARBALL:-}" \
    "${CRATE}/../../target/wb-llama-build/llama.cpp-${TAG}.tar.gz" \
    "${HOME}/wb-bench/llama.cpp/llama.cpp-${TAG}-src.tar.gz"; do
    if [[ -n "${candidate}" && -f "${candidate}" && "$(sha_of "${candidate}")" == "${SRC_SHA256}" ]]; then
      TARBALL="${candidate}"
      break
    fi
  done
  if [[ ! -f "${TARBALL}" ]]; then
    echo "build.sh: fetching ${SRC_URL}" >&2
    curl --fail --location --proto '=https' --tlsv1.2 --silent --show-error \
      --output "${TARBALL}.tmp" "${SRC_URL}"
    mv "${TARBALL}.tmp" "${TARBALL}"
  fi
  if [[ "$(sha_of "${TARBALL}")" != "${SRC_SHA256}" ]]; then
    echo "build.sh: source sha256 mismatch for ${TARBALL}" >&2
    exit 1
  fi
  rm -rf "${SRC}.tmp"
  mkdir -p "${SRC}.tmp"
  tar -xzf "${TARBALL}" -C "${SRC}.tmp" --strip-components 1
  mv "${SRC}.tmp" "${SRC}"
fi

# --- build -------------------------------------------------------------------
if [[ ! -f "${BUILD}/.wb-flags" || "$(cat "${BUILD}/.wb-flags")" != "${CMAKE_FLAGS[*]}" ]]; then
  rm -rf "${BUILD}"
fi
mkdir -p "${BUILD}"
if ! env -u HF_UI_VERSION -u HF_WEBUI_VERSION \
  cmake -S "${HERE}" -B "${BUILD}" -DWB_LLAMA_SRC="${SRC}" "${CMAKE_FLAGS[@]}" \
  >"${OUT}/configure.log" 2>&1; then
  tail -n 40 "${OUT}/configure.log" >&2
  exit 1
fi
printf '%s' "${CMAKE_FLAGS[*]}" >"${BUILD}/.wb-flags"
if ! cmake --build "${BUILD}" --config Release --target wb_llm \
  -j "${WB_LLAMA_JOBS:-$(sysctl -n hw.logicalcpu)}" >"${OUT}/build.log" 2>&1; then
  grep -E "error|Error" "${OUT}/build.log" | head -n 40 >&2
  tail -n 20 "${OUT}/build.log" >&2
  exit 1
fi

# --- one object, only wb_llm_* visible ---------------------------------------
# `ld -r` merges our objects and the archive members they need into a single
# relocatable object; with an export list every other global symbol (ggml_*,
# gguf_*, llama_*, the C++ ones, the embedded Metal library) becomes local to
# it, so it cannot meet transcribe-cpp's ggml at the final link.
ARCHIVES=()
while IFS= read -r a; do
  [[ "${a}" == */libwb_llm.a ]] || ARCHIVES+=("${a}")
done < <(find "${BUILD}" -name '*.a' | sort)
rm -f "${OUT}/wb_llm_all.o" "${LIB}.tmp"
xcrun --sdk "${SDK}" ld -r -arch arm64 \
  -platform_version "${LD_PLATFORM}" "${MIN}" "$(xcrun --sdk "${SDK}" --show-sdk-version)" \
  -exported_symbols_list "${HERE}/exports.txt" \
  -force_load "${BUILD}/libwb_llm.a" "${ARCHIVES[@]}" \
  -o "${OUT}/wb_llm_all.o"
xcrun --sdk "${SDK}" libtool -static -o "${LIB}.tmp" "${OUT}/wb_llm_all.o" 2>/dev/null

# --- checks -------------------------------------------------------------------
# 1. Nothing but the API is visible.
visible="$(xcrun nm -gU "${LIB}.tmp" 2>/dev/null | awk 'NF >= 3 {print $3}' | sort)"
if [[ "${visible}" != "$(sort "${HERE}/exports.txt")" ]]; then
  echo "build.sh: the visible symbols are not exactly exports.txt:" >&2
  head -n 20 <<<"${visible}" >&2
  exit 1
fi
leaked="$(grep -v '^_wb_llm_' <<<"${visible}" || true)"
if [[ -n "${leaked}" ]]; then
  echo "build.sh: symbols other than wb_llm_* are visible:" >&2
  head -n 20 <<<"${leaked}" >&2
  exit 1
fi
# 2. Nothing of llama.cpp or ggml is left for the final link to resolve (it
#    would bind to transcribe-cpp's ggml).
unresolved="$(xcrun nm -u "${LIB}.tmp" 2>/dev/null | grep -E '_(ggml|gguf|llama|common|mtmd|clip)_' || true)"
if [[ -n "${unresolved}" ]]; then
  echo "build.sh: unresolved llama.cpp symbols:" >&2
  head -n 20 <<<"${unresolved}" >&2
  exit 1
fi
# 3. The Objective-C class was renamed.
if xcrun nm "${LIB}.tmp" 2>/dev/null | grep -q 'OBJC_CLASS_\$_GGMLMetalClass'; then
  echo "build.sh: GGMLMetalClass was not renamed" >&2
  exit 1
fi

mv "${LIB}.tmp" "${LIB}"
stamp_text >"${OUT}/stamp"
echo "${OUT}"

#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the `llama-server` sidecar (R5 §4.5, W-04, S-10) from the pinned
# llama.cpp tag: static (no dylibs), Metal shaders embedded, macOS 13.0
# deployment target, no TLS, no web UI, no network code paths at build time
# beyond fetching the pinned source tarball from GitHub (sha256-checked).
#
# Output: src-tauri/target/wb-sidecar/llama-server-<target triple>, the
# `bundle.externalBin` entry of src-tauri/tauri.fork.conf.json. Tauri copies it
# next to the app binary (dev) or into Contents/MacOS (bundle) and signs it
# with the app's identity when it bundles.
#
# Usage:
#   ci/llama-server/build.sh               # build (reuses the cached source)
#   ci/llama-server/build.sh --if-missing  # no-op when the output matches the pin
#   ci/llama-server/build.sh --print-path  # print the output path and exit
#
# Env:
#   WB_LLAMA_SRC_TARBALL  local copy of the source tarball (still sha256-checked)
#   WB_LLAMA_JOBS         parallel build jobs (default: logical CPUs)
#   WB_TARGET             aarch64-apple-darwin or x86_64-apple-darwin (default:
#                         this Mac's own; an Apple Silicon Mac builds the Intel
#                         one too, and runs its checks under Rosetta 2)
#
# BLAS (Accelerate's new cblas_sgemm) is off: that symbol is macOS 13.3+ and
# would be an unguarded call on 13.0-13.2. The Metal backend does the work;
# the CPU fallback (`-ngl 0`) uses ggml's own kernels.
#
# Apple Silicon: Metal, and CPU code for an M1 baseline (armv8.4-a + fp16 +
# dotprod, no i8mm/SME), never `-mcpu=native`, so a binary built on a newer
# Mac still runs on an M1.
# Intel: no Metal (on Intel/AMD GPUs ggml-metal can hang compiling its
# shaders, Handy #2340, and is rarely faster than the CPU for a 2B model);
# CPU code for AVX2 + FMA + F16C (Haswell), which every Intel Mac that runs
# macOS 13 has. The app starts this sidecar with `-ngl 0` (process.rs).
set -euo pipefail

TAG="b11624"
COMMIT="af0579c78a07a555d84830d01da7b61cfadbc7ef"
SRC_URL="https://github.com/ggml-org/llama.cpp/archive/refs/tags/${TAG}.tar.gz"
SRC_SHA256="9a44be53ab3c76ab958131e8d9f2e14ab1e885cf1841afdcd77ca936c066249f"
DEPLOYMENT_TARGET="13.0"
CPU_ARCH="armv8.4-a+fp16+dotprod"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT}/src-tauri/target/wb-sidecar"
CACHE_DIR="${ROOT}/src-tauri/target/wb-llama-build"

if [[ -n "${WB_TARGET:-}" ]]; then
  TRIPLE="${WB_TARGET}"
else
  case "$(uname -m)" in
    arm64 | aarch64) TRIPLE="aarch64-apple-darwin" ;;
    x86_64) TRIPLE="x86_64-apple-darwin" ;;
    *) TRIPLE="unknown" ;;
  esac
fi
case "${TRIPLE}" in
  aarch64-apple-darwin)
    OSX_ARCH=arm64
    GPU_FLAGS=(-DGGML_METAL=ON -DGGML_METAL_EMBED_LIBRARY=ON)
    CPU_FLAGS=(-DGGML_CPU_ARM_ARCH="${CPU_ARCH}")
    ;;
  x86_64-apple-darwin)
    OSX_ARCH=x86_64
    GPU_FLAGS=(-DGGML_METAL=OFF)
    CPU_FLAGS=(
      -DGGML_SSE42=ON
      -DGGML_AVX=ON
      -DGGML_AVX2=ON
      -DGGML_FMA=ON
      -DGGML_F16C=ON
      -DGGML_BMI2=ON
      -DGGML_AVX512=OFF
    )
    ;;
  *)
    echo "build.sh: macOS target aarch64-apple-darwin or x86_64-apple-darwin only; got ${TRIPLE}" >&2
    exit 1
    ;;
esac
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "build.sh: macOS only" >&2
  exit 1
fi

OUT="${OUT_DIR}/llama-server-${TRIPLE}"
STAMP="${OUT}.stamp"

CMAKE_FLAGS=(
  -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_OSX_DEPLOYMENT_TARGET="${DEPLOYMENT_TARGET}"
  -DCMAKE_OSX_ARCHITECTURES="${OSX_ARCH}"
  -DBUILD_SHARED_LIBS=OFF
  "${GPU_FLAGS[@]}"
  -DGGML_NATIVE=OFF
  "${CPU_FLAGS[@]}"
  -DGGML_RPC=OFF
  -DGGML_BLAS=OFF
  -DGGML_BACKEND_DL=OFF
  -DLLAMA_OPENSSL=OFF
  -DLLAMA_BUILD_BORINGSSL=OFF
  -DLLAMA_BUILD_UI=OFF
  -DLLAMA_USE_PREBUILT_UI=OFF
  -DLLAMA_BUILD_TESTS=OFF
  -DLLAMA_BUILD_EXAMPLES=OFF
  -DLLAMA_BUILD_TOOLS=ON
  -DLLAMA_BUILD_SERVER=ON
  -DLLAMA_BUILD_APP=OFF
  -DLLAMA_BUILD_NUMBER="${TAG#b}"
  -DLLAMA_BUILD_COMMIT="${COMMIT:0:9}"
)

# The stamp pins everything that changes the binary.
stamp_text() {
  printf 'tag=%s\ncommit=%s\nsrc_sha256=%s\nflags=%s\n' \
    "${TAG}" "${COMMIT}" "${SRC_SHA256}" "${CMAKE_FLAGS[*]}"
}

case "${1:-}" in
  --print-path)
    echo "${OUT}"
    exit 0
    ;;
  --if-missing)
    if [[ -x "${OUT}" && -f "${STAMP}" ]] && [[ "$(cat "${STAMP}")" == "$(stamp_text)" ]]; then
      exit 0
    fi
    ;;
  "") ;;
  *)
    echo "usage: $0 [--if-missing | --print-path]" >&2
    exit 2
    ;;
esac

command -v cmake >/dev/null || {
  echo "build.sh: cmake is required (brew install cmake)" >&2
  exit 1
}

mkdir -p "${CACHE_DIR}" "${OUT_DIR}"
TARBALL="${CACHE_DIR}/llama.cpp-${TAG}.tar.gz"

sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }

if [[ -n "${WB_LLAMA_SRC_TARBALL:-}" ]]; then
  cp "${WB_LLAMA_SRC_TARBALL}" "${TARBALL}.tmp"
  mv "${TARBALL}.tmp" "${TARBALL}"
fi
if [[ ! -f "${TARBALL}" || "$(sha_of "${TARBALL}")" != "${SRC_SHA256}" ]]; then
  echo "build.sh: fetching ${SRC_URL}"
  curl --fail --location --proto '=https' --tlsv1.2 --silent --show-error \
    --output "${TARBALL}.tmp" "${SRC_URL}"
  mv "${TARBALL}.tmp" "${TARBALL}"
fi
got="$(sha_of "${TARBALL}")"
if [[ "${got}" != "${SRC_SHA256}" ]]; then
  echo "build.sh: source sha256 mismatch: got ${got}, want ${SRC_SHA256}" >&2
  rm -f "${TARBALL}"
  exit 1
fi

SRC="${CACHE_DIR}/src-${TAG}"
BUILD="${CACHE_DIR}/build-${TAG}"
[[ "${TRIPLE}" == aarch64-apple-darwin ]] || BUILD="${BUILD}-${TRIPLE}"
if [[ ! -f "${SRC}/CMakeLists.txt" ]]; then
  rm -rf "${SRC}" "${SRC}.tmp"
  mkdir -p "${SRC}.tmp"
  tar -xzf "${TARBALL}" -C "${SRC}.tmp" --strip-components 1
  mv "${SRC}.tmp" "${SRC}"
fi

# A fresh configure whenever the flags change (CMake caches option values).
if [[ ! -f "${BUILD}/.wb-flags" || "$(cat "${BUILD}/.wb-flags")" != "$(stamp_text)" ]]; then
  rm -rf "${BUILD}"
fi
mkdir -p "${BUILD}"
# Offline environment for the build: no proxy-provided endpoints, no HF
# variables the UI step could read.
if ! env -u HF_UI_VERSION -u HF_WEBUI_VERSION \
  cmake -S "${SRC}" -B "${BUILD}" "${CMAKE_FLAGS[@]}" >"${BUILD}/configure.log" 2>&1; then
  tail -n 40 "${BUILD}/configure.log" >&2
  exit 1
fi
stamp_text >"${BUILD}/.wb-flags"
if ! cmake --build "${BUILD}" --config Release --target llama-server \
  -j "${WB_LLAMA_JOBS:-$(sysctl -n hw.logicalcpu)}" >"${BUILD}/build.log" 2>&1; then
  tail -n 40 "${BUILD}/build.log" >&2
  exit 1
fi
# An unguarded call to an API newer than the deployment target would crash
# on older macOS versions: fail the build instead.
if grep -q "is only available on macOS" "${BUILD}/build.log"; then
  grep "is only available on macOS" "${BUILD}/build.log" >&2
  echo "build.sh: API newer than macOS ${DEPLOYMENT_TARGET} used without a guard" >&2
  exit 1
fi

BIN="${BUILD}/bin/llama-server"
[[ -x "${BIN}" ]] || {
  echo "build.sh: ${BIN} was not produced (see ${BUILD}/build.log)" >&2
  exit 1
}

# --- checks -----------------------------------------------------------------
# 1. Static: only system libraries and frameworks.
bad_libs="$(otool -L "${BIN}" | tail -n +2 | awk '{print $1}' |
  grep -v -E '^(/usr/lib/|/System/Library/)' || true)"
if [[ -n "${bad_libs}" ]]; then
  echo "build.sh: non-system dylibs linked: ${bad_libs}" >&2
  exit 1
fi
# 2. Deployment target.
minos="$(vtool -show-build "${BIN}" 2>/dev/null | awk '/minos/ {print $2; exit}')"
if [[ "${minos}" != "${DEPLOYMENT_TARGET}" ]]; then
  echo "build.sh: minos ${minos:-?}, want ${DEPLOYMENT_TARGET}" >&2
  exit 1
fi
# 3. The architecture asked for (no Metal code in the Intel one).
if ! lipo -archs "${BIN}" | grep -qx "${TRIPLE%%-*}"; then
  echo "build.sh: ${BIN} is $(lipo -archs "${BIN}"), want ${TRIPLE%%-*}" >&2
  exit 1
fi
if [[ "${TRIPLE}" == x86_64-apple-darwin ]] && otool -L "${BIN}" | grep -q Metal.framework; then
  echo "build.sh: the Intel sidecar links Metal" >&2
  exit 1
fi
# 4. It runs (Rosetta 2 for Intel on Apple Silicon) and reports the pinned build.
version="$("${BIN}" --version 2>&1 || true)"
if ! grep -q "build ${TAG#b}" <<<"${version}"; then
  echo "build.sh: unexpected --version output: ${version}" >&2
  exit 1
fi

cp "${BIN}" "${OUT}.tmp"
# Ad-hoc signature with the hardened runtime, the same flags the app bundle
# uses (tauri.conf.json `hardenedRuntime`). A release build re-signs the
# sidecar with the Developer ID during bundling (F-02); no entitlements are
# needed (verified in RS-06: Metal shader compilation works under the
# hardened runtime without allow-jit / allow-unsigned-executable-memory).
codesign --force --sign - --identifier com.wisprbro.desktop.llama-server \
  --options runtime --timestamp=none "${OUT}.tmp"
mv "${OUT}.tmp" "${OUT}"
stamp_text >"${STAMP}"
echo "build.sh: ${OUT}"
echo "build.sh: $(head -n 2 <<<"${version}" | tr '\n' ' ')"

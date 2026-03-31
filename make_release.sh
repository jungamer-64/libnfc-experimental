#! /bin/sh

set -eu

PROJECT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
BUILD_DIR="${PROJECT_DIR}/build/release"

LIBNFC_VERSION=$(awk '
  $1 == "project(libnfc" {
    for (i = 1; i <= NF; ++i) {
      if ($i == "VERSION") {
        print $(i + 1)
        exit
      }
    }
  }
' "${PROJECT_DIR}/CMakeLists.txt")

if [ -z "${LIBNFC_VERSION}" ]; then
  echo "Could not determine libnfc version from CMakeLists.txt" >&2
  exit 1
fi

echo "=== Building release artifacts for libnfc ${LIBNFC_VERSION} ==="
rm -rf "${BUILD_DIR}"

cmake -S "${PROJECT_DIR}" -B "${BUILD_DIR}" -DBUILD_TESTING=ON
cmake --build "${BUILD_DIR}"
ctest --test-dir "${BUILD_DIR}" --output-on-failure

echo "=== Building source package ==="
cpack --config "${BUILD_DIR}/CPackSourceConfig.cmake"

echo "=== Building binary package ==="
cpack --config "${BUILD_DIR}/CPackConfig.cmake"

echo "Artifacts are available in ${BUILD_DIR}"

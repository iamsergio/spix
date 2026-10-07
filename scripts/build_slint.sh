#!/usr/bin/env bash
# Build Slint from source, install it into build-slint/install, then configure and build Spix
# with the Slint scene library against it (preset dev-slint).
#
# Usage: scripts/build_slint.sh <slint-source-dir> [extra cmake --build args...]
#
# Expects the 3rdparty/vcpkg submodule to be checked out.
set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <slint-source-dir> [extra cmake --build args...]" >&2
    exit 1
fi
slint_src="$(realpath "$1")"
shift

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

for f in "$slint_src/api/cpp/CMakeLists.txt" 3rdparty/vcpkg/scripts/buildsystems/vcpkg.cmake; do
    if [[ ! -e "$f" ]]; then
        echo "error: $f not found" >&2
        exit 1
    fi
done

# EXPERIMENTAL, TESTING and SYSTEM_TESTING are required by Spix::Slint. MCP makes
# SLINT_BACKEND=headless available, so the tests run without a display.
cmake -S "$slint_src/api/cpp" -B build-slint -G Ninja \
    -DCMAKE_BUILD_TYPE=Debug \
    -DCMAKE_INSTALL_PREFIX="$root/build-slint/install" \
    -DSLINT_FEATURE_EXPERIMENTAL=ON \
    -DSLINT_FEATURE_TESTING=ON \
    -DSLINT_FEATURE_SYSTEM_TESTING=ON \
    -DSLINT_FEATURE_MCP=ON \
    -DSLINT_FEATURE_INTERPRETER=OFF \
    -DSLINT_FEATURE_SYSTEM_TRAY=OFF \
    -DSLINT_FEATURE_GETTEXT=OFF
cmake --build build-slint
cmake --install build-slint

cmake --preset dev-slint
cmake --build build-dev-slint "$@"

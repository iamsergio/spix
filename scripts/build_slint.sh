#!/usr/bin/env bash
# Configure and build Spix with the Slint scene library (preset dev-slint).
#
# Usage: scripts/build_slint.sh [extra cmake --build args...]
#
# Expects the 3rdparty/slint and 3rdparty/vcpkg submodules to be checked out.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

for f in 3rdparty/slint/api/cpp/CMakeLists.txt 3rdparty/vcpkg/scripts/buildsystems/vcpkg.cmake; do
    if [[ ! -e "$f" ]]; then
        echo "error: $f not found, check out the submodules first" >&2
        exit 1
    fi
done

cmake --preset dev-slint
cmake --build build-dev-slint "$@"

#!/bin/sh
# Ko, (Baczynski-Bruce) 2026; Apache 2.0
#
# Builds, runs, and tests the crepe project in a copy, so build/ stays out
# of the source tree.
# Usage: build.sh <output dir>
set -eu
out=$1
crepe=${CREPE:-crepe}
here=$(cd "$(dirname "$0")" && pwd)
rm -rf "$out/project"
mkdir -p "$out/project"
cp -R "$here/mathx" "$here/app" "$out/project/"
cd "$out/project/app"
"$crepe" build 2>/dev/null
"$crepe" test >/dev/null 2>&1
"$crepe" tree >/dev/null
test -f build/debug/include/app.h
test -f build/debug/libapp.a
./build/debug/fractions
./build/debug/wordcount
./build/debug/legacy-tool
"$crepe" run --target fractions 2>/dev/null

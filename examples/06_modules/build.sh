#!/bin/sh
# Ko, (Baczynski-Bruce) 2026; Apache 2.0
#
# Builds the example two ways: in one invocation, and one module at a time
# the way crepe does, with interfaces in a shared directory.
# Usage: build.sh <output dir>
set -eu
out=$1
crust=${CRUST:-crust}
cd "$(dirname "$0")"
"$crust" app.crm shapes.crm geometry.crm units.crm -o "$out/app" -lm
mkdir -p "$out/modules"
for m in units geometry shapes app; do
    "$crust" -c "$m.crm" -o "$out/modules/$m.o"
done
"$crust" "$out/modules/app.o" "$out/modules/shapes.o" "$out/modules/geometry.o" "$out/modules/units.o" \
    -o "$out/app_separate" -lm
"$crust" --print-interface "$out/modules/shapes.crmi" > "$out/shapes.interface"
"$out/app_separate" > "$out/separate.stdout"
"$out/app"

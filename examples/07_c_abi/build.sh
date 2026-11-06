#!/bin/sh
# Ko, (Baczynski-Bruce) 2026; Apache 2.0
#
# Builds the Crust library three ways and links a C program against each.
# Usage: build.sh <output dir>
set -eu
out=$1
crust=${CRUST:-crust}
cc=${CC:-cc}
cd "$(dirname "$0")"
"$crust" --emit=header vecmath.crm -o "$out/vecmath.h"         # the C header
"$crust" --emit=staticlib vecmath.crm -o "$out/libvecmath.a"   # a static library
"$crust" -shared vecmath.crm -o "$out/libvecmath.so"           # a shared library
"$cc" -I"$out" main.c "$out/libvecmath.a" -o "$out/with_static"
"$cc" -I"$out" main.c -L"$out" -lvecmath -Wl,-rpath,"$out" -o "$out/with_shared"
"$crust" -I"$out" main.c vecmath.crm -o "$out/with_crust"      # crust compiles the C too
"$out/with_shared" > "$out/shared.stdout"
"$out/with_crust" > "$out/crust.stdout"
cmp "$out/shared.stdout" "$out/crust.stdout"
"$out/with_static"

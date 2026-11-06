#!/bin/sh
# Ko, (Baczynski-Bruce) 2026; Apache 2.0
#
# The crust command line beyond `cc` (docs/CLI.md, section 1).
# Usage: build.sh <output dir>
set -eu
out=$1
crust=${CRUST:-crust}
cd "$(dirname "$0")"
mkdir -p "$out/deps/textutil"

# A dependency: object and interface, as crepe builds them.
"$crust" -c textutil.crm -o "$out/deps/textutil/textutil.o"
"$crust" --print-interface "$out/deps/textutil/textutil.crmi" | sed -n '1p;4,$p'

# An importer finds it through --extern.
"$crust" tool.crm "$out/deps/textutil/textutil.o" --extern textutil="$out/deps/textutil" -o "$out/tool"
"$out/tool"

# Other outputs.
"$crust" --emit=llvm-ir tool.crm --extern textutil="$out/deps/textutil" -o "$out/tool.ll"
"$crust" --emit=asm tool.crm --extern textutil="$out/deps/textutil" -o "$out/tool.s"
"$crust" --emit=interface textutil.crm -o "$out/textutil.crmi"
test -s "$out/tool.ll" && test -s "$out/tool.s" && test -s "$out/textutil.crmi"

# Diagnostics as JSON, one object per line.
if "$crust" legacy.c --error-format=json -o "$out/legacy" 2> "$out/legacy.json"; then
    echo "expected legacy.c to be rejected"
    exit 1
fi
grep -o '"code":"CR0381"' "$out/legacy.json"

# --unchecked: migration only.
"$crust" --unchecked legacy.c -o "$out/legacy" 2>/dev/null
"$out/legacy"

# --std picks the language mode regardless of the extension.
"$crust" --std=c23 keywords.c -o "$out/keywords"
"$out/keywords"
if "$crust" --std=crust keywords.c -o "$out/keywords_crust" 2>/dev/null; then
    echo "expected keywords.c to fail as Crust"
    exit 1
fi

# The long form of a diagnostic code.
"$crust" --explain CR0381 | head -n 1

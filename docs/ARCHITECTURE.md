<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Architecture

Status: implemented by Crust 0.1.1. The repo layout below is the real one. Related: [MEMORY_SAFETY.md](MEMORY_SAFETY.md), [ABI.md](ABI.md), [CLI.md](CLI.md).

## 1. Pipeline

```
source (.c .h .cr .hr .crm)
  │  preprocess         C23 #include, #define, #if, #embed, __has_include
  ▼
tokens ──► parse ──► AST
                      │  module graph: resolve imports against .crmi files and this build's modules
                      │  name resolution, type checking, const checking
                      ▼
                 CrIR  (control-flow graph; moves, borrows, drops explicit)
                      │
        ┌─────────────┼────────────────────┬──────────────────┐
        ▼             ▼                    ▼                  ▼
  move & init    borrow & lifetime     drop elaboration   bounds obligations
        └─────────────┴────────────────────┴──────────────────┘
                      │  all pass, or diagnostics and stop
                      ▼
            lower to LLVM IR
                      │  LLVM passes: bounds traps, invariant checks, debug sanitizers
                      ▼
            LLVM optimizer  ──►  object files  ──►  link  ──►  exe | .a | .so | .h
```

Stages after the AST never print. They return diagnostics to the driver.

## 2. Stages

| Stage | Input | Output | Notes |
|---|---|---|---|
| Preprocess | files, `-D`, `-I` | token stream | C23 preprocessor. Keep it standard. `#include` of a `.hr` header inserts Crust text, which is parsed in Crust mode. |
| Lex | tokens | tokens with spans | Lifetimes `'a` are lexed as their own token. The lexer tells them apart from `'a'` by checking for a closing quote. |
| Parse | tokens | AST | Accepts C23 in `.c` and `.h`. Accepts Crust in `.cr`, `.hr`, and `.crm`. The `module` and `export` forms are valid only in `.crm`. |
| Module graph | ASTs | module graph, resolved imports | Module names unique, dependencies listed, no cycles. Loads `.crmi` for imported modules. See MODULES.md. |
| Sema | AST | typed AST | Name resolution, types, const expressions, attribute validation, exhaustive `switch`. |
| Lower to CrIR | typed AST | CrIR | Makes moves, borrows, drops, and storage lifetimes explicit. |
| Safety analyses | CrIR | proofs or diagnostics | See MEMORY_SAFETY.md, section 3. |
| Lower to LLVM | CrIR | LLVM IR | Carries proof obligations as metadata or as inserted checks. |
| LLVM passes | LLVM IR | LLVM IR | Bounds traps, invariant verification, debug sanitizers. |
| Codegen | LLVM IR | object | Target from `--target`. Optimization level from `-O`. |
| Emit interface | typed AST | `.crmi` file | Written per module after sema. Its hash changes only when exports change. |
| Link | objects | exe, `.a`, `.so`, headers | System linker (`cc`, `ld`, or `lld`). Headers come from `--emit=header`. |

## 3. Diagnostics

- Every diagnostic has a span (file, line, column, length), a severity, a stable code `CRnnnn`, a message, and optional notes and a fix hint.
- Codes are stable. Once a code ships, its meaning does not change. Tests match on codes.
- `crust --explain CRnnnn` prints the long form for each code.
- The safety analyses report the two sites involved in an error.

## 4. Repo layout

```
src/
  driver/     crust entry point, option parsing, pipeline wiring
  diag/       spans, diagnostics, codes, rendering
  pp/         preprocessor
  lex/        lexer
  parse/      parser and AST
  sema/       name resolution, types, const checks, attributes
  crir/       CrIR definition and lowering from AST
  safety/     move, init, borrow, lifetime, drop, and bounds analyses
  codegen/    CrIR to LLVM IR, LLVM pass pipeline, object emission
  abi/        target layouts and calling conventions
  modules/    module graph, import resolution, .crmi reading and writing
crepe/        crepe entry point and dependency resolution (separate binary)
ensure/       fluent test assertions, `ensure(x).cannot_be(y)` (TESTING.md, section 9)
lib/include/  freestanding C headers that ship with crust (stddef.h, stdarg.h, ...)
tests/        compile_fail/, run_pass/, modules/, pairs/, soundness/, abi/, interface/
tools/        check_style (header and naming check)
docs/         this documentation
```

Modules depend downward only: `driver` → everything; `sema` must not depend on `codegen`; `safety` must not depend on `codegen`. Rules that matter here are enforced in review and, later, by a checker script.

## 5. Implementation language

Rust (D-019), as one Cargo workspace with two binaries, `crust` and `crepe`. Self-hosting in Crust is a later goal. The naming rules in [CODE_STYLE.md](CODE_STYLE.md) need the lint allowances in the root `Cargo.toml`. The compiler has no crate dependencies. The workspace also holds `ensure`, a test-only assertion library that nothing ships.

LLVM 18 (D-024). `src/codegen` writes textual LLVM IR, and the driver runs LLVM 18's `clang` to turn it into objects and to link. Building crust needs no LLVM libraries. The IR stays inside LLVM's stable textual format, so the pin is one major version.

Each `src/` directory is a Rust module. The stages communicate through plain data: tokens, the AST (`parse::ast`), the typed IR (`sema::tir`), CrIR (`crir`), and LLVM IR text. Diagnostics go to a sink that the driver renders.

## 6. Build outputs

Build output goes to `build/`, which is git-ignored. `crepe` puts its artifacts there. A direct `crust` invocation writes to the path given with `-o`, and intermediate files go next to that path only when `--emit` asks for them.

## 7. crepe

`crepe` does not compile code itself. It:

1. Reads the manifest (`crepe.toml`, D-025).
2. Resolves dependencies to exact versions, and records them in `crepe.lock`.
3. Fetches sources (path, git, and later a registry) into a cache.
4. Calls `crust` for each package in dependency order, passing include paths, link paths, and `--extern` directories holding the `.crmi` interfaces of its dependencies.
5. Writes the final artifacts, plus the generated headers that C consumers need.

Crust knows about modules, interface files, include paths, and link paths. It does not know about packages. `crepe` maps packages onto `--extern` directories. Keeping that boundary means `crust` works without `crepe`.

## 8. External dependencies

- LLVM 18 and its `clang` (D-024)
- A system linker (`cc`, `ld`, or `lld`)
- A C library for the target (libc)
- For tests: an existing C compiler, to compare ABI behavior against

## 9. Incremental builds

The one incremental build in v1 is at module granularity. An importer rebuilds only when an imported `.crmi` changes (see [MODULES.md](MODULES.md), section 6). `.cr` sources have no interface file, so a change to one rebuilds every target that compiles it. There is no compile server and no distributed build in v1. Revisit those when compile times are a measured problem, not a guessed one.

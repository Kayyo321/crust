<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Architecture

Status: draft. The repo layout is a plan. Create directories in M0, not before. Related: [MEMORY_SAFETY.md](MEMORY_SAFETY.md), [ABI.md](ABI.md), [CLI.md](CLI.md).

## 1. Pipeline

```
source (.c .h .cr)
  │  preprocess         C23 #include, #define, #if, #embed, __has_include
  ▼
tokens ──► parse ──► AST
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
| Preprocess | files, `-D`, `-I` | token stream | C23 preprocessor. Keep it standard. |
| Lex | tokens | tokens with spans | Lifetimes `'a` are lexed as their own token. The lexer tells them apart from `'a'` by checking for a closing quote. |
| Parse | tokens | AST | Accepts C23 and, in `.cr` mode, Crust additions. |
| Sema | AST | typed AST | Name resolution, types, const expressions, attribute validation, exhaustive `switch`. |
| Lower to CrIR | typed AST | CrIR | Makes moves, borrows, drops, and storage lifetimes explicit. |
| Safety analyses | CrIR | proofs or diagnostics | See MEMORY_SAFETY.md, section 3. |
| Lower to LLVM | CrIR | LLVM IR | Carries proof obligations as metadata or as inserted checks. |
| LLVM passes | LLVM IR | LLVM IR | Bounds traps, invariant verification, debug sanitizers. |
| Codegen | LLVM IR | object | Target from `--target`. Optimization level from `-O`. |
| Link | objects | exe, `.a`, `.so`, headers | System linker (`cc`, `ld`, or `lld`). Headers come from `--emit=header`. |

## 3. Diagnostics

- Every diagnostic has a span (file, line, column, length), a severity, a stable code `CRnnnn`, a message, and optional notes and a fix hint.
- Codes are stable. Once a code ships, its meaning does not change. Tests match on codes.
- `crust --explain CRnnnn` prints the long form for each code.
- The safety analyses report the two sites involved in an error.

## 4. Repo layout (planned for M0)

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
crepe/        crepe entry point and dependency resolution (separate binary)
lib/          minimal runtime and standard library in Crust
tests/        compile_fail/, run_pass/, soundness/, conformance/, abi/
tools/        style check, header check, coverage report
docs/         this documentation
```

Modules depend downward only: `driver` → everything; `sema` must not depend on `codegen`; `safety` must not depend on `codegen`. Rules that matter here are enforced in review and, later, by a checker script.

## 5. Implementation language

**(open, D-019).** Proposed: Rust, bootstrapped first, with self-hosting in Crust as a later goal. `.gitignore` is a Cargo template, which suggests Rust was already the intent. If Rust is chosen, the naming rules in [CODE_STYLE.md](CODE_STYLE.md) need the lint allowances listed there.

The LLVM binding is also open (D-024). Options: the LLVM C API directly, or a Rust wrapper over it. Either way, the compiler pins one LLVM major version.

## 6. Build outputs

Build output goes to `build/`, which is git-ignored. `crepe` puts its artifacts there. A direct `crust` invocation writes to the path given with `-o`, and intermediate files go next to that path only when `--emit` asks for them.

## 7. crepe

`crepe` does not compile code itself. It:

1. Reads the manifest (`crepe.toml`, D-025).
2. Resolves dependencies to exact versions, and records them in `crepe.lock`.
3. Fetches sources (path, git, and later a registry) into a cache.
4. Calls `crust` for each package in dependency order, passing include and link paths.
5. Writes the final artifacts, plus the generated headers that C consumers need.

Crust does not know about packages. It knows about files, include paths, and link paths. Keeping that boundary means `crust` works without `crepe`.

## 8. External dependencies

- LLVM (exact major version pinned, D-024)
- A system linker (`cc`, `ld`, or `lld`)
- A C library for the target (libc)
- For tests: an existing C compiler, to compare ABI behavior against

## 9. What we don't build yet

No incremental compilation, no compile server, and no distributed builds in v1. Revisit when compile times are a measured problem, not a guessed one.

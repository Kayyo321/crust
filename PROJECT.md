<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Crust: project definition

Status: draft. Crust 0.1.1 is implemented (see the milestone notes below). Owner: Ko (Baczynski-Bruce). Decisions and open questions live in [docs/DECISIONS.md](docs/DECISIONS.md).

## Vision

C is the lingua franca of systems software, and it is unsafe by construction. Rust showed that most memory errors can be ruled out at compile time without giving up performance. Crust brings those rules to C. You keep C23 and its ABI, add a few constructs where the checker needs them, and get a compiler that can guarantee memory safety for the code it checks.

## Problem

- Existing C code leaks memory, frees memory twice, reads freed memory, and overflows buffers. Existing tools only find some of these, and only when they run.
- Hand-written headers duplicate every declaration. The copies drift apart, every file re-parses them, and macros leak between them. Crust projects use modules, which generate their interfaces instead.
- Rewriting C in Rust is expensive, and it breaks the C ABI and existing build systems.
- Bolting safety onto C with a separate language or a heavy runtime loses most of C's reach.

## Goals

| # | Goal | Measure |
|---|---|---|
| G1 | **C23 compatibility.** Every valid C23 translation unit compiles in C mode. | Pass rate on a C23 conformance suite, tracked from M2 |
| G2 | **Memory safety in checked code.** No use-after-free, double free, leak, out-of-bounds access, or uninitialized read. | Soundness corpus passes; sanitizer runs clean |
| G3 | **Full C ABI compatibility.** Crust code and C code link and call each other. | ABI test matrix passes on each supported target |
| G4 | **Low friction.** `let`, type inference, and clear diagnostics. Annotations only where the checker needs them. | Diagnostics carry a code, a span, and a fix hint |
| G5 | **Dependencies like cargo.** Resolve, fetch, and build libraries through `crepe`. | `crepe build` works on a multi-package project (M7) |
| G6 | **Native performance.** Use LLVM. Checks cost nothing where the compiler can prove them away. | Benchmark overhead tracked from M5 |
| G7 | **Modules for projects.** Multi-file Crust projects use `.crm` modules, which generate their interfaces and share builds across targets. `.cr` with `.hr` headers stays supported for old-school code. | Modules are the default in examples and templates. Importers rebuild only when an interface changes (M6). |

## Non-goals

- Not a C++ compiler. No classes, templates, exceptions, or RTTI.
- No garbage collector and no unwinding. Failed runtime checks trap.
- Not a new language with its own syntax family. Crust is a superset of C.
- Not a replacement for the C preprocessor in v1.
- Not a guarantee about code inside `unsafe` blocks, or about C code the programmer links in. Those are the programmer's proof obligations. See [docs/MEMORY_SAFETY.md](docs/MEMORY_SAFETY.md).

## Principles

1. **Safe by default, unsafe on purpose.** Checked code is the default. Unsafe operations must be explicit and local.
2. **Superset first.** Prefer C23 attributes over new keywords. Every new keyword is a break for some C program.
3. **State guarantees precisely.** Write down what is guaranteed and what is not. Don't overclaim.
4. **Fast feedback.** Tests stay fast. Coverage is a floor, not a score to chase.
5. **Docs are the spec.** Code follows the docs. Changed behavior changes the docs in the same change.
6. **Modules for projects.** The compiler generates a module's interface from its source, so nobody maintains a second copy of a declaration. Old-school `.cr` and `.hr` pairs stay supported, but new projects use modules. See [docs/MODULES.md](docs/MODULES.md).

## What we take from Rust

| Rust rule | In Crust |
|---|---|
| Single owner, move semantics | Owned values move on assignment and passing; use after move is an error |
| Aliasing XOR mutability | Many shared borrows or one unique borrow |
| Lifetimes | Borrows cannot outlive their owner; `'a` annotations where elision fails |
| Definite initialization | Reading an unassigned variable is an error |
| Drop at scope exit | Owned values are released on every path; this gives leak freedom |
| Bounds-checked indexing | Checked where the bound is known; a trap otherwise |
| Exhaustive `match` | Exhaustive `switch` over enums in checked code |

The full mapping, with milestones, is in [docs/MEMORY_SAFETY.md](docs/MEMORY_SAFETY.md).

## Toolchain at a glance

- **`crust`**: the compiler. Parses C23, Crust source (`.cr`), Crust headers (`.hr`), and Crust modules (`.crm`), checks memory safety, lowers to LLVM, emits objects, executables, static and shared libraries, `.crmi` module interfaces, and C headers. See [docs/CLI.md](docs/CLI.md) and [docs/MODULES.md](docs/MODULES.md).
- **`crepe`**: the package manager. Manages dependencies, lockfiles, and builds, and calls `crust`. See [docs/CLI.md](docs/CLI.md).
- **LLVM** for code generation and optimization. The system linker for final linking.

## Milestones

Each milestone ends with an exit criterion, not a date.

Crust 0.1.1 covers M0, most of M1–M7 for x86-64 Linux, and the first part of the safety core: the analyses run on CrIR, a three-module program builds and runs, a C program links a Crust library, and a multi-package crepe project with several targets builds. Not yet: the C23 conformance suite, coverage gates, AArch64, the LLVM invariant-verification pass, and the benchmark set.

| Milestone | Scope | Exit criterion |
|---|---|---|
| M0 Foundations | Repo layout, build system, CI, header and naming checks, a program that compiles through LLVM | CI green on an empty program that prints "hello" |
| M1 C23 front end | Preprocessor, lexer, parser, and semantic analysis for C23 | Parses the C23 conformance suite with no spurious errors |
| M2 C codegen | C23 to LLVM IR; C-compat ABI for scalars, structs, and pointers | Run-pass tests match clang output for the supported subset |
| M3 Crust surface | `let`, `mut`, `unsafe`, lifetime syntax, `[[crust::...]]` attributes, `.cr` and `.hr` source/header pairs, `.crm` modules (`module`, `import`, `export`) within one build, diagnostics with codes | Compile-fail tests for each syntax rule; a three-module program builds and runs |
| M4 Safety core | Crust IR; move and init analysis; borrow and lifetime checking; drop elaboration | Soundness corpus: every must-reject case rejected, every must-run case runs leak-free |
| M5 Bounds and slices | Slice types, bounds checks, C-compatible lowering of Crust types | Benchmark overhead measured and within the target set at M5 |
| M6 Libraries | Static and shared library output; `.crmi` interface files with the rebuild rule; generated C headers for C consumers | A C program links against a Crust library and passes the ABI matrix; editing a function body does not rebuild importers |
| M7 crepe | Manifest, lockfile, path and git dependencies, build targets, build through `crust` | A multi-package project with several build targets builds from a clean checkout |
| M8 Later | Registry, threads (`Send`/`Sync`), generics, self-hosting exploration | Scoped when M7 is complete |

## Success metrics

- **Soundness:** zero known cases where checked code leaks, double-frees, uses freed memory, or overflows, and zero entries in the soundness corpus that the checker wrongly accepts.
- **Compatibility:** C23 conformance pass rate, and the list of C programs that fail to compile in C mode, both tracked each release.
- **Cost:** runtime overhead of inserted checks on the benchmark set, reported each release.

## Glossary

- **Checked code:** code the Crust checker analyzes. This is all code in `.c`, `.cr`, and `.crm` files outside `unsafe` blocks, unless compiled with `--unchecked`.
- **Unsafe code:** code inside an `unsafe` block or `unsafe` function, where the programmer takes on the proof obligations.
- **Owned value:** a value with a single owner that is released when the owner goes out of scope.
- **Borrow:** a reference that does not own its target. Shared borrows are read-only. Unique borrows allow mutation.
- **Lifetime:** the region of code during which a borrow is valid.
- **Slice:** a pointer plus a length. Indexing a slice is bounds-checked.
- **Trap:** a deliberate, immediate program abort with a diagnostic when a runtime check fails.
- **CrIR:** Crust IR, the mid-level representation where the safety analyses run. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
- **Soundness bug:** the checker accepts a program that violates a guarantee in checked code.
- **Module:** one `.crm` file, declared with `module`. Its exports are its public surface. See [docs/MODULES.md](docs/MODULES.md).
- **Interface file (`.crmi`):** the compiler-generated description of a module's exports. Importers read it instead of the source.
- **Source/header pair:** a `.cr` file and the `.hr` header it includes. The old-school form, still supported.

## Open questions

The full list is in [docs/DECISIONS.md](docs/DECISIONS.md). The biggest ones:

1. Whether all C undefined behavior is eliminated in checked code, or only the kinds that break memory safety (D-021).
2. Integer overflow in checked code (D-022). 0.1 wraps in the meantime (D-042).
3. Whether nullable pointers get an option-like type in checked code (D-023).
4. Struct fields that hold borrows (D-048).

Decided for 0.1.0: borrows are spelled `ref` (D-018), the compiler is written in Rust (D-019), and it targets LLVM 18 through textual IR (D-024).

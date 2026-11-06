<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Decisions

The log of design decisions and open questions. Each entry has one of three statuses:

- **Accepted:** decided. The docs describe it as settled.
- **Proposed:** a recommendation. The docs describe it, and it may change on review. Implement only after it is accepted.
- **Open:** undecided. The docs name the question. Don't implement it.

Adding or changing an entry is a PR, the same as any doc change. Statuses change only when the decision is made, and the change is recorded in the same PR.

## Accepted

| ID | Decision | Where it's described |
|---|---|---|
| D-001 | Crust is a superset of C23. Its checked mode guarantees memory safety using Rust's rules. | PROJECT.md, LANGUAGE.md, MEMORY_SAFETY.md |
| D-002 | Two CLIs: `crust` (compiler) and `crepe` (package manager, like cargo). | CLI.md |
| D-003 | Full C ABI compatibility. Crust-only types cross into C only in their C-compatible form, and only through `[[crust::c_abi]]` exports (amended by D-034). | ABI.md |
| D-004 | `let` introduces bindings. | LANGUAGE.md |
| D-005 | Lifetimes annotate borrows. | LANGUAGE.md, MEMORY_SAFETY.md |
| D-006 | Naming: `snake_case` everywhere, except constants and enums, which are `PascalCase`. Never `camelCase`. | CODE_STYLE.md |
| D-007 | Every file starts with the signature header (CODE_STYLE.md, section 1). | CODE_STYLE.md |
| D-020 | `.crm` modules are the modern way to organize Crust projects: many files, several build targets, shared libraries. A module declares its name with `module`, exports with `export`, and imports with TypeScript-style `import`. `.cr` source with `.hr` headers stays for old-school code, and headers stay for C. | MODULES.md, section 1 |

## Proposed

| ID | Recommendation | Why | Notes |
|---|---|---|---|
| D-008 | Ownership, borrow, lifetime, initialization, and drop analyses run on Crust IR (CrIR) before LLVM. LLVM passes insert bounds traps, verify invariants, and provide debug sanitizers. | LLVM IR has no ownership or borrow information, and Rust does its borrow check on its own IR for this reason. | Differs from "all as LLVM passes". Confirm. |
| D-009 | Failed runtime checks trap (abort with a diagnostic). No unwinding. | C has no unwinding. Keeps the runtime small. | |
| D-010 | Leak freedom is defined for checked code via unique ownership, no forget-like operation in checked code, and shared ownership deferred. Statics and memory live at exit are excluded. | Rust does not give leak freedom. This definition is one we can enforce. | MEMORY_SAFETY.md, section 4 |
| D-011 | `let` bindings are immutable by default. `let mut` is mutable. Plain C declarations stay mutable. | Immutability by default makes aliasing easier to check. | LANGUAGE.md, section 3 |
| D-012 | File kinds: `.c` and `.h` are C23 with checks on. `.cr` is Crust source, `.hr` is a Crust header included by text, and `.crm` is a Crust module. All three are Crust with checks on. `let`, `mut`, `unsafe`, `module`, `export`, and `import` are reserved in every Crust file. `--unchecked` is for migration. | Keeps C files as C. The reserved words are the one place Crust is not a strict superset, and it's deliberate. | LANGUAGE.md, section 2; MODULES.md, section 1 |
| D-013 | Crust extensions use C23 vendor attributes, `[[crust::...]]`, before new keywords. | Attributes cannot break C programs. | LANGUAGE.md, section 10 |
| D-014 | Raw pointers are tracked when their origin is visible (a local, a slice, a borrow). Other raw pointers need `unsafe` to dereference. | Keeps ordinary C checked where possible, and forces an explicit `unsafe` where the origin is unknown. | MEMORY_SAFETY.md, section 5 |
| D-015 | Naming and formatting enforced by `tools/check_style` and `clang-format` (or `rustfmt`). | Rules that can be checked by tools are not left to review. | CODE_STYLE.md, section 8 |
| D-016 | Trunk-based git. `main` stays green. Short-lived branches for review. | Fewest moving parts for a small team. | CONTRIBUTING.md |
| D-017 | Coverage floors: 80% overall lines; 90% lines and 85% branches in `src/safety/`. Ratchet: no drop greater than 0.5 points. Gated from M2. | Enough to catch regressions without chasing 100%. | TESTING.md, section 4 |
| D-028 | Module names are dotted, with snake_case segments, and unique per build. The file path convention (`src/net/http.crm` for `net.http`) is a lint warning, not an error. Package roots are named after the package. | Names stay stable when files move, and identity does not depend on directory layout. | MODULES.md, section 3 |
| D-029 | Import specifiers: `"name"` for a module in the same package, `"package/name"` for a dependency. Only packages listed under `[dependencies]` in `crepe.toml` can be imported. | Mirrors npm subpaths, and keeps dependency edges explicit. | MODULES.md, section 5 |
| D-030 | Each compiled module produces a `.crmi` interface file. Importers rebuild only when an imported interface hash changes, not when a function body changes. | Replaces headers, and gives the one incremental build in v1. | MODULES.md, section 6 |
| D-031 | Import cycles are an error in v1. Every module-level variable takes a constant initializer, so importing a module runs no code. | Avoids initialization-order bugs, which cycles make easy to write. | MODULES.md, section 7 |
| D-032 | Macros do not cross module boundaries. `#include` of a `.crm` file is an error. `.h` headers included from Crust are parsed as C, so `export`, `module`, and `import` stay identifiers inside them. `.hr` headers are Crust, and their macros reach the includer, as in any textual include. `.c` files cannot `import`. | Keeps C headers from breaking Crust keywords, keeps C consumers on generated headers, and keeps the old-school textual behavior where it is wanted. | MODULES.md, section 8 |
| D-033 | v1 export surface: named exports, export lists, `export ... from`, `export *`, and namespace imports. No default exports, dynamic imports, or side-effect imports. | The smallest TypeScript-like surface that covers real use. | MODULES.md, section 11 |
| D-034 | Two kinds of export. `export` is for Crust modules: any Crust type, mangled symbol. `[[crust::c_abi]] export` uses the C ABI: C-compatible types only, plain symbol, listed in the generated header. Amends D-003. | Crust-only types stay Crust-only, without breaking C callers. | MODULES.md, section 10; ABI.md |
| D-035 | Source/header pairs: a `.cr` file includes its `.hr` header as text. The compiler checks every definition's signature against its header declaration, and a mismatch is an error. Pairs have no interface file. | Keeps the old-school style working without silent signature drift. | MODULES.md, sections 8 and 9 |
| D-036 | Build targets are declared in `crepe.toml` as `[[bin]]` and `[[lib]]` tables. A target names a `.crm` root, or lists `.cr` sources with `include` directories for `.hr` headers. Each `.crm` module compiles once per configuration and is shared by every target that imports it. | Large projects with many targets build each module once, and each target reuses it. | MODULES.md, section 7; CLI.md |

## Open

| ID | Question | Options | Blocks |
|---|---|---|---|
| D-018 | Surface syntax of references, and whether `&x` means a borrow or an address-of in Crust code (`.cr`, `.crm`). Also the syntax of slices. | (a) `&x` is a borrow in `.cr` and `.crm`, address-of in `.c`. (b) Borrows use a new spelling everywhere. | M3 |
| D-019 | Implementation language: Rust, as proposed, or C. | Rust matches the `.gitignore`. C matches the C ABI work more directly. Self-hosting is later either way. | M0 |
| D-021 | Which C undefined behavior is eliminated in checked code. | (a) Only what breaks memory safety. (b) All C UB, as traps. | M4 |
| D-022 | Integer overflow in checked code. | (a) Trap. (b) Wrap, with explicit `wrapping_` operations. | M3 |
| D-023 | Nullable checked pointers. | (a) Option-like type. (b) Keep raw pointers with a null check before dereference. | M5 |
| D-024 | LLVM major version and the binding to use. | Pin the newest version the team can maintain. Binding: C API directly, or a wrapper. | M0 |
| D-025 | `crepe` manifest name and registry strategy. | Manifest: `crepe.toml`, as proposed. Registry: git and path first, then a hosted registry. | M7 |
| D-026 | Generics in v1. | Proposed: no, deferred. Generics are needed for libraries, but they add a large checker surface. | M8 |
| D-027 | Whether `crepe` supports language editions, so the language can change without breaking old packages. | Editions add a versioning scheme. Decide before 1.0. | 1.0 |

## Known soundness bugs

None. Entries are added here when a soundness bug is confirmed, with its status. Fixed entries stay for the record.

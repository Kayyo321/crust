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
| D-008 | Ownership, borrow, lifetime, initialization, and drop analyses run on Crust IR (CrIR) before LLVM. LLVM passes insert bounds traps, verify invariants, and provide debug sanitizers. | ARCHITECTURE.md, MEMORY_SAFETY.md, section 7 |
| D-009 | Failed runtime checks trap (abort with a diagnostic). No unwinding. | MEMORY_SAFETY.md, section 6 |
| D-010 | Leak freedom is defined for checked code via unique ownership, no forget-like operation in checked code, and shared ownership deferred. Statics and memory live at exit are excluded. | MEMORY_SAFETY.md, section 4 |
| D-011 | `let` bindings are immutable by default. `let mut` is mutable. Plain C declarations stay mutable. | LANGUAGE.md, section 3 |
| D-012 | File kinds: `.c` and `.h` are C23 with checks on. `.cr` is Crust source, `.hr` is a Crust header included by text, and `.crm` is a Crust module. All three are Crust with checks on. `let`, `mut`, `unsafe`, `module`, `export`, `import`, and `ref` (added by D-018) are reserved in every Crust file. `--unchecked` is for migration. | LANGUAGE.md, section 2; MODULES.md, section 1 |
| D-013 | Crust extensions use C23 vendor attributes, `[[crust::...]]`, before new keywords. | LANGUAGE.md, section 10 |
| D-014 | Raw pointers are tracked when their origin is visible (a local, a slice, a borrow). Other raw pointers need `unsafe` to dereference. | MEMORY_SAFETY.md, section 5 |
| D-018 | Borrows use a new spelling in every mode (option b). `ref x` is a shared borrow and `ref mut x` a unique one. Borrow types are `ref T`, `ref mut T`, `ref 'a T`, and `ref 'a mut T`. Slices are `ref [T]` and `ref mut [T]`, with `.len` and bounds-checked indexing; a borrow of an array converts to a slice. `&x` is always C address-of, which gives a raw pointer. `ref` is reserved in Crust files. | LANGUAGE.md, sections 5 and 6 |
| D-019 | The compiler and crepe are written in Rust, as one Cargo workspace. Self-hosting is a later goal. | ARCHITECTURE.md, section 5 |
| D-020 | `.crm` modules are the modern way to organize Crust projects: many files, several build targets, shared libraries. A module declares its name with `module`, exports with `export`, and imports with TypeScript-style `import`. `.cr` source with `.hr` headers stays for old-school code, and headers stay for C. | MODULES.md, section 1 |
| D-024 | LLVM 18. crust writes textual LLVM IR and runs LLVM 18's `clang` to make objects and to link, so building crust needs no LLVM development libraries. `CRUST_LLVM_CONFIG` selects the installation. | ARCHITECTURE.md, sections 5 and 8 |
| D-025 | The manifest is `crepe.toml`. Dependencies come from paths and git in 0.1.0. The registry question moved to D-043. | CLI.md, section 2 |
| D-028 | Module names are dotted, with snake_case segments, and unique per build. The file path convention (`src/net/http.crm` for `net.http`) is a lint warning, not an error. Package roots are named after the package. | MODULES.md, section 3 |
| D-029 | Import specifiers: `"name"` for a module in the same package, `"package/name"` for a dependency. Only packages listed under `[dependencies]` in `crepe.toml` can be imported. | MODULES.md, section 5 |
| D-030 | Each compiled module produces a `.crmi` interface file. Importers rebuild only when an imported interface hash changes, not when a function body changes. | MODULES.md, section 6 |
| D-031 | Import cycles are an error in v1. Every module-level variable takes a constant initializer, so importing a module runs no code. | MODULES.md, section 7 |
| D-032 | Macros do not cross module boundaries. `#include` of a `.crm` file is an error. `.h` headers included from Crust are parsed as C, so `export`, `module`, and `import` stay identifiers inside them. `.hr` headers are Crust, and their macros reach the includer, as in any textual include. `.c` files cannot `import`. | MODULES.md, section 8 |
| D-033 | v1 export surface: named exports, export lists, `export ... from`, `export *`, and namespace imports. No default exports, dynamic imports, or side-effect imports. | MODULES.md, section 11 |
| D-034 | Two kinds of export. `export` is for Crust modules: any Crust type, mangled symbol. `[[crust::c_abi]] export` uses the C ABI: C-compatible types only, plain symbol, listed in the generated header. Amends D-003. | MODULES.md, section 10; ABI.md |
| D-035 | Source/header pairs: a `.cr` file includes its `.hr` header as text. The compiler checks every definition's signature against its header declaration, and a mismatch is an error. Pairs have no interface file. | MODULES.md, sections 8 and 9 |
| D-036 | Build targets are declared in `crepe.toml` as `[[bin]]` and `[[lib]]` tables. A target names a `.crm` root, or lists `.cr` sources with `include` directories for `.hr` headers. Each `.crm` module compiles once per configuration and is shared by every target that imports it. | MODULES.md, section 7; CLI.md |

## Proposed

| ID | Recommendation | Why | Notes |
|---|---|---|---|
| D-015 | Naming and formatting enforced by `tools/check_style` and `rustfmt` (`clang-format` for C sources, when there are any). | Rules that can be checked by tools are not left to review. | CODE_STYLE.md, section 8 |
| D-016 | Trunk-based git. `main` stays green. Short-lived branches for review. | Fewest moving parts for a small team. | CONTRIBUTING.md |
| D-017 | Coverage floors: 80% overall lines; 90% lines and 85% branches in `src/safety/`. Ratchet: no drop greater than 0.5 points. Gated from M2. | Enough to catch regressions without chasing 100%. | TESTING.md, section 4 |
| D-037 | C-mode files (`.c`, `.h`) have no `unsafe` gating. Operations that need `unsafe` in Crust files (calling C functions, dereferencing untracked pointers, pointer arithmetic, unrelated casts) are allowed in C mode, and are outside the guarantee. Every other check stays on. | C files cannot spell `unsafe`, so gating would make ordinary C uncompilable without `--unchecked`. Chosen by the project owner for 0.1.0. | MEMORY_SAFETY.md, sections 1 and 2; LANGUAGE.md, section 2 |
| D-038 | A non-owned local whose address is taken, or that is written element by element or through a union member, is zero-filled when its function starts. Such writes then count as initialization. | Keeps C idioms (`scanf("%d", &x)`, filling an array in a loop) without ever reading indeterminate bytes. | MEMORY_SAFETY.md, section 4 |
| D-039 | `crust -c` on a `.crm` file also writes `<module>.crmi` next to the object. Same-package imports resolve to modules given in the same invocation, then to `<module>.crmi` in the directory of `-o`. `crust` also accepts `.o`, `.a`, and `.so` inputs to link, as `cc` does. | crepe compiles one module at a time and needs a place for interfaces. No new flag is needed. | CLI.md, section 1; MODULES.md, section 7 |
| D-040 | Owned types: a drop function has type `void f(struct T *)` or `void f(ref mut struct T)`. After it runs, the owned members are dropped. Owned values cannot be union members, cannot be built field by field, and cannot be moved out of a pointer, an element, a global, or a field of a type with a drop function. | Matches Rust's drop order, and keeps every owned value either whole or moved. | LANGUAGE.md, sections 4 and 8 |
| D-041 | Borrow and pointer rules in 0.1.0: borrows cannot be struct or union members, array elements, or globals. Raw pointers to locals conflict only with moves and with the local's scope ending, not with reads or writes. Storing a local's address in a global, or through an untracked pointer in Crust code, is an error. Calls through function pointers need `unsafe`. | Sound without lifetime parameters on structs, which v1 does not have (generics are deferred, D-026). | MEMORY_SAFETY.md, section 5 |
| D-042 | Interim integer semantics while D-022 is open: arithmetic wraps (no undefined behavior), shift counts are taken modulo the width, signed division overflow wraps, integer division by zero traps, and float-to-integer conversion saturates. | Something has to be chosen, and these choices never make checked code undefined. D-022 can still pick trapping. | LANGUAGE.md, section 11 |
| D-044 | Crust 0.1.0 supports one target: x86-64 Linux (System V). AArch64 Linux follows. | The ABI work was done and tested against clang for this target first. | ABI.md, section 2 |
| D-045 | `crepe run` also takes `--release` and `--target <name>`, and crepe reads `[[test]]` tables like `[[bin]]` for `crepe test`. | `crepe run` needs to pick a binary and a profile like `crepe build`; tests are targets (MODULES.md, section 7). | CLI.md, section 2 |
| D-046 | Enumerators of an exported enum are exported with it, as constants. They are imported by name, like any other export, or reached through a namespace import. | Keeps the rule that an import names what it brings in. | MODULES.md, section 5 |

## Open

| ID | Question | Options | Blocks |
|---|---|---|---|
| D-021 | Which C undefined behavior is eliminated in checked code. | (a) Only what breaks memory safety. (b) All C UB, as traps. | M4 |
| D-022 | Integer overflow in checked code. 0.1.0 wraps in the meantime (D-042). | (a) Trap. (b) Wrap, with explicit `wrapping_` operations. | M3 |
| D-023 | Nullable checked pointers. | (a) Option-like type. (b) Keep raw pointers with a null check before dereference. | M5 |
| D-026 | Generics in v1. | Proposed: no, deferred. Generics are needed for libraries, but they add a large checker surface. | M8 |
| D-027 | Whether `crepe` supports language editions, so the language can change without breaking old packages. | Editions add a versioning scheme. Decide before 1.0. | 1.0 |
| D-043 | crepe registry strategy (split from D-025). | Git and path only until a hosted registry exists; or a git-index registry. | M8 |
| D-047 | A package with both `src/main.crm` and `src/lib.crm`: both roots must be named after the package (D-028), but module names must be unique per build. | (a) The binary root takes the name `main`. (b) The library root is `<package>` and the binary root `<package>.main`. (c) Allow the clash only between a bin root and a lib root. | M7 |
| D-048 | Struct fields that hold borrows, which need lifetime parameters on structs. | Add `struct view<'a>`-style lifetime parameters, or wait for generics (D-026). | M8 |

## Known soundness bugs

None. Entries are added here when a soundness bug is confirmed, with its status. Fixed entries stay for the record.

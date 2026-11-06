<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Memory safety in Crust

Status: draft; implemented by Crust 0.1.1 as described, with the limits noted below. This document is the contract. If the compiler behaves differently, the compiler has a soundness bug. Related: [LANGUAGE.md](LANGUAGE.md), [ARCHITECTURE.md](ARCHITECTURE.md), [DECISIONS.md](DECISIONS.md).

## 1. The guarantee

For **checked code**, Crust guarantees that a running program never:

| ID | Violation | Mechanism |
|---|---|---|
| G-UAF | Uses memory after it is freed, or through a dangling reference | Lifetimes and borrows |
| G-DF | Frees the same memory twice | Single ownership, move checking |
| G-LEAK | Loses the last reference to owned memory without releasing it | Drop elaboration on every path (section 4) |
| G-OOB | Reads or writes outside an array or slice | Bounds checks, proved or trapped |
| G-UNINIT | Reads a value that was never initialized | Definite-initialization analysis |
| G-ALIAS | Mutates a value while a shared borrow of it is live | Borrow checking |

Checked code is every `.c`, `.cr`, or `.crm` function body outside `unsafe`, compiled without `--unchecked`. In `.c` files, the operations that need `unsafe` in Crust files are allowed and are outside the guarantee (D-037); see section 2.

Planned, not in the first guarantee set: G-NULL (dereference of a possibly null checked pointer), and G-RACE (data races between threads, after M7).

## 2. What the guarantee does not cover

The guarantee holds only if the **trusted base** holds. The items below are outside it. Where Crust itself is responsible for an item, a failure there is a soundness bug.

- **Unsafe code.** `unsafe` blocks are the programmer's proof obligations. Each one carries a `SAFETY:` comment.
- **C code linked in.** Crust cannot check the body of a C function. Calling it is an `unsafe` operation unless declared `[[crust::safe]]`.
- **Module contracts.** A safe export must uphold the invariants its module relies on. The checker cannot verify that. See [MODULES.md](MODULES.md), section 9.
- **Unchecked mode.** `--unchecked` turns checks off for a translation unit.
- **C-mode operations.** In `.c` and `.h` files, calling C functions, dereferencing pointers of unknown origin, pointer arithmetic, and unrelated casts are allowed without `unsafe` (D-037). The guarantee covers the rest of a C file's checks, not those operations.
- **The compiler and LLVM.** A miscompilation of checked code is a soundness bug. It is not an exception to the guarantee.
- **Hardware, OS, libc, and the linker.**
- **Program exit and program-lifetime data.** Memory owned by statics, and memory still allocated when `main` returns, is not a leak. See section 4.

## 3. Rust rules and their mechanisms

| Rust rule | Crust mechanism | Stage | Milestone |
|---|---|---|---|
| Move semantics | Move analysis on CrIR | Analysis | M4 |
| Definite initialization | Dataflow on CrIR | Analysis | M4 |
| Aliasing XOR mutability | Borrow checker on CrIR | Analysis | M4 |
| Lifetimes outlive uses | Region inference on CrIR | Analysis | M4 |
| Drop at scope exit | Drop elaboration on CrIR | Lowering | M4 |
| Bounds checking | Static proof, else runtime check that traps | Analysis, then LLVM | M5 |
| Exhaustive `match` | Exhaustive `switch` on enums | Sema | M3 |
| Non-null references | `&'a T` is never null by construction | Sema | M3 |
| `Send` / `Sync` | Thread-safety markers | Sema | M8 |

## 4. Leak freedom

"Zero memory leaks" needs a precise definition. Rust does not give leak freedom: `mem::forget` and reference-counted cycles are safe Rust. Crust gives it for checked code by construction:

1. Owned values have a single owner, so ownership forms a tree, not a graph. A tree cannot contain a cycle.
2. Every owned value is dropped at the end of its scope, or moved. Drop elaboration inserts the drop on every exit path, including early returns and `goto`.
3. Checked code has no forget-like operation. Ownership can end only by a move, a drop, or a call to a function that consumes it.
4. Shared ownership (a reference-counted type) is not in v1. When it is added, cycles through it are either rejected or allowed only inside `unsafe`.

So in checked code, every allocation is released or moved on every path.

Definite initialization (G-UNINIT) follows Rust for locals that are read whole. A non-owned local whose address is taken, or that is written element by element or through a union member, is zero-filled when its function starts, and such writes count as initialization (D-038). Its bytes are never indeterminate, so C idioms such as `scanf("%d", &x)` and filling an array in a loop stay valid. Exceptions: memory owned by statics or still live when `main` returns, and memory leaked inside `unsafe`. The soundness corpus checks this with LeakSanitizer on the run-pass tests.

## 5. Raw pointers

C pointers `T *` are the hardest part of checked code. Crust tracks where a pointer points when it can:

- **Tracked pointers** have a known origin: the address of a live local, a slice, or a borrow. The checker treats a tracked pointer like the borrow it came from. Dereferencing it is checked.
- **Untracked pointers** come from anywhere else: `malloc`, a C function, an integer cast, arithmetic. Dereferencing one requires `unsafe`.

This keeps ordinary C code checked where the origin is visible, and forces an explicit `unsafe` where it is not.

How 0.1 tracks pointers (D-041):

- Every value that can hold a pointer carries a set of origins: the locals it may point into, the lifetime regions of borrowed parameters, static data, or unknown memory. The sets follow assignments, struct members, calls, and stores through tracked pointers.
- A pointer to a local keeps that local borrowed while any live value may hold it. The local cannot be moved out of or go out of scope during that time (CR0505, CR0597). Unlike a `ref` borrow, a raw pointer does not stop reads or writes of the local; aliasing raw pointers is C, not a memory error.
- A pointer to a local cannot be stored in a global (CR0521), or returned (CR0515). In Crust files it cannot be stored through a pointer the checker does not track (CR0521).
- Dereferencing a raw pointer is checked only when every origin is known. A parameter, a `malloc` result, the result of pointer arithmetic, or a pointer read from memory the checker does not track needs `unsafe` (CR0133).

## 6. Bounds

- Array indexes with a constant in range: no check.
- Indexes the compiler can prove in range: no check. Proof uses the same analysis as borrowing, and LLVM's own range analysis can help.
- Everything else: a runtime check that traps on failure.
- Slices are always checked against their length.
- Raw pointers have no bounds. Indexing one is pointer arithmetic and needs `unsafe` in Crust code.

A trap prints a diagnostic with the source location and aborts. There is no unwinding (D-009). Overhead is measured on the benchmark set from M5.

## 7. Where checks run

Checks run in two places. The split is deliberate. See [ARCHITECTURE.md](ARCHITECTURE.md) and D-008.

**Analyses on CrIR (the source of truth).** Move, initialization, borrow, lifetime, drop, and bounds-obligation analyses run on Crust IR, a control-flow graph that keeps ownership, borrows, and drops explicit. LLVM IR does not carry ownership or borrow information. Rust also runs its borrow checker on its own mid-level IR, not on LLVM IR, for this reason.

**LLVM passes (defense in depth).** LLVM passes do three things:

1. Insert the runtime bounds checks and traps that the CrIR analyses could not discharge.
2. Verify that the emitted IR still satisfies the invariants the CrIR analyses proved. This catches lowering bugs.
3. Provide sanitizer-style checks in debug builds (`-g` builds).

A pass failure in step 2 is a compiler bug, and it blocks the build.

In 0.1, bounds checks and division-by-zero checks are inserted when the AST is lowered to CrIR, so the analyses see them, and LLVM receives them as ordinary branches to a trap. Step 2 is not implemented yet. With `-g`, AddressSanitizer and LeakSanitizer are enabled when clang's sanitizer runtime is installed. The test suite also runs every run-pass program under valgrind's leak checker when it is available.

## 8. Diagnostics

Every safety error has a stable code (`CRnnnn`, see [ARCHITECTURE.md](ARCHITECTURE.md)), names the rule, points at the two sites involved (for example, the move and the later use), and gives a fix hint where one exists. `crust --explain CRnnnn` prints the long form.

## 9. Soundness

**Definition.** A soundness bug is a program that the checker accepts, that is checked code, and that exhibits G-UAF, G-DF, G-LEAK, G-OOB, G-UNINIT, or G-ALIAS, or a miscompilation that causes one.

**Policy.**

- Soundness bugs are the highest priority. See [SECURITY.md](SECURITY.md) for private reporting.
- Every fix adds a case to the soundness corpus (see [TESTING.md](TESTING.md)) before the fix lands.
- A known soundness bug is recorded in DECISIONS.md with its status, even if it is not yet fixed. A bug is never silently accepted.
- A rule may be weakened only by a DECISIONS.md entry that says so, with the reason.

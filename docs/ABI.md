<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# C ABI compatibility

Status: implemented by Crust 0.1.1 for x86-64 Linux. Goal G3: full C ABI compatibility. Related: [LANGUAGE.md](LANGUAGE.md), [ARCHITECTURE.md](ARCHITECTURE.md), [DECISIONS.md](DECISIONS.md) (D-003, amended by D-034), [MODULES.md](MODULES.md).

## 1. The rule

For any type, function, or variable that has C linkage or appears in a C-visible signature, Crust matches what the target's C compiler does. In practice that means clang or gcc on the same target. Specifically:

- Struct, union, and array layout, including padding, alignment, bit-fields, and `_Atomic` layout, match.
- Enum size and underlying type follow C23 rules, including fixed underlying types such as `enum : int`.
- Scalar sizes and `long double` match the target.
- Calling convention, argument passing, struct returns, and varargs match the target's psABI.
- Symbol names are not mangled for C-linkage functions and variables.

Crust does not add a runtime ABI requirement on C callers. A C program calling a Crust library needs no Crust runtime beyond what the library itself links.

## 2. Targets

| Target | Status |
|---|---|
| x86-64 System V (Linux, BSD) | Milestone M2. Linux supported by 0.1 (D-044). |
| AArch64 AAPCS (Linux) | Milestone M2. Not in 0.1. |
| x86-64 System V (macOS) | Milestone M6 |
| Windows x64 | After M7, scope TBD |

Targets are named by LLVM target triple, passed with `--target`.

## 3. Crust-only types across the boundary

Some Crust types have no C equivalent: slices, borrows, and owned values. They are allowed in Crust code. They can cross the C boundary only in their C-compatible lowering, and the compiler checks that.

| Crust type | C-compatible form at the boundary |
|---|---|
| `&'a T`, `&'a mut T` | `T *` (lifetime erased; the caller owns the proof) |
| Slice of `T` | Struct of `T *` and `size_t`, in that field order |
| Owned value | `T *`, with release through the type's drop function, which must be exported |

Rules:

1. An export marked `[[crust::c_abi]]` may use only C-compatible types in its signature. Using a Crust-only type is a compile error, with a fix hint that names the C-compatible form.
2. Other exports and private declarations may use any Crust type. They use the Crust ABI, which is mangled by module name (see [MODULES.md](MODULES.md), section 10).
3. Passing a borrow to C erases its lifetime. The compiler does not check what the C code does with it. Any such pass is inside `unsafe` unless the callee is declared `[[crust::safe]]`.

## 4. Declaring and exporting

- C headers declare external C functions. Calling them is `unsafe` unless declared `[[crust::safe]]` (see LANGUAGE.md, section 9).
- A `[[crust::c_abi]] export` function is exported under its own name, with no mangling. Other exports are mangled by module name.
- `--emit=header` writes a C23 header for every `[[crust::c_abi]]` export. C programs include that header. The header contains no Crust syntax.

## 5. Verification

The ABI matrix is part of the test suite (see [TESTING.md](TESTING.md)):

1. **Layout tests.** For each C type, a C file and a Crust file each compute `sizeof`, `_Alignof`, and `offsetof`. Both are compared against values from the target C compiler.
2. **Call tests, both directions.** A C caller calls a Crust callee, and a Crust caller calls a C callee. Each test covers scalars, structs by value (small and large), struct returns, varargs, and function pointers.
3. **Library tests.** A C program links a Crust static and a Crust shared library, using the generated header.

A failing ABI test blocks the release, the same as a soundness failure.

The matrix lives in `tests/abi/` and runs with `cargo test --test abi`. It compiles the C half with clang, and compares layouts and call results.

## 6. Stability

- Before 1.0, ABI changes are allowed, but each one needs a DECISIONS.md entry and a release note.
- From 1.0, the ABI is stable within a major version. Breaking changes need a new major version.

## 7. What is not promised

- ABI compatibility of Crust-only types with other Crust versions, before 1.0.
- ABI of code compiled with `--unchecked`, beyond what C provides. Unchecked code is still C-ABI compatible.
- Compatibility with compilers other than the target's C compiler, where the psABI is ambiguous.

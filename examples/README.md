<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Crust examples

Each folder shows one part of what Crust adds to C23. Every program here builds and runs with Crust 0.1.1, and `cargo test --test examples` keeps it that way. Comments in each file point to the section of the docs that defines the feature.

Build `crust` and `crepe` first:

```bash
cargo build --workspace
```

Then, from a folder, run the command in the comment at the top of its main file, with `crust` meaning `target/debug/crust`. Folders with a `build.sh` run it: `CRUST=../../target/debug/crust CREPE=../../target/debug/crepe sh build.sh /tmp/out`.

| Folder | Shows | Docs |
|---|---|---|
| [01_let](01_let) | `let`, `let mut`, `let x: T`, `let x;` with the type from the first assignment | LANGUAGE.md §3 |
| [02_borrows](02_borrows) | `ref x`, `ref mut x`, borrows ending at their last use, reborrows, tracked `&x` pointers; lifetime elision and `'a` annotations | LANGUAGE.md §5 |
| [03_slices](03_slices) | `ref [T]`, `ref mut [T]`, `.len`, arrays becoming slices, compile-time and run-time bounds checks | LANGUAGE.md §6 |
| [04_ownership](04_ownership) | `[[crust::drop(f)]]`, moves, drops on every path, drop flags, owned members, drop order | LANGUAGE.md §4, §8 |
| [05_unsafe](05_unsafe) | `unsafe` blocks and functions, `SAFETY:` comments, `[[crust::safe]]`, untracked pointers, union reads, function pointers, module-level `let mut` | LANGUAGE.md §9 |
| [06_modules](06_modules) | `module`, every `import` and `export` form, `ns::name`, `struct ns::tag`, enum and typedef exports, `.crmi` interfaces, building module by module | MODULES.md |
| [07_c_abi](07_c_abi) | `[[crust::c_abi]] export`, the generated C header, static and shared libraries used from C | MODULES.md §10, ABI.md |
| [08_pairs](08_pairs) | Old-school `.cr` source with an `.hr` header, header macros, signature checking | MODULES.md §8 |
| [09_mixing_c](09_mixing_c) | A `.c` file in the same program, C headers parsed as C, the C library and libm | LANGUAGE.md §2 |
| [10_switch](10_switch) | Exhaustive `switch` over enums, fixed underlying types | MEMORY_SAFETY.md §3 |
| [11_crepe_project](11_crepe_project) | `crepe.toml`, a path dependency, `[[bin]]`, `[[lib]]`, `[[test]]`, a `.cr` target, shared modules, `crepe build`, `run`, `test`, `tree` | CLI.md §2 |
| [12_safety_errors](12_safety_errors) | Programs the checker rejects, one file per area: use after free, double free, aliasing, mutability, initialization, bounds, `unsafe`, lifetimes, the C ABI boundary | MEMORY_SAFETY.md |
| [13_toolchain](13_toolchain) | `--extern`, `--print-interface`, `--emit` kinds, `--error-format=json`, `--unchecked`, `--std`, `--explain` | CLI.md §1 |

In [12_safety_errors](12_safety_errors), the comment `//~ ERROR CRnnnn` at the end of a line names the error the compiler reports there. `crust --explain CRnnnn` prints the long form.

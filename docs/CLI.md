<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Command-line tools

Status: implemented by Crust 0.1.1. Flags can still change before 1.0, where the stable interface starts. Related: [ARCHITECTURE.md](ARCHITECTURE.md), [ABI.md](ABI.md).

Two binaries:

- **`crust`**: the compiler. Like `rustc`. It compiles one set of inputs to one output.
- **`crepe`**: the package manager. Like `cargo`. It manages packages and calls `crust`.

## 1. `crust`

```
crust [options] <input>...
```

Input files are `.c` (C), `.cr` (Crust source), and `.crm` (Crust modules, see MODULES.md). `.h` and `.hr` headers are included, not compiled alone. `.o`, `.a`, and `.so` inputs are passed to the linker, as with `cc` (D-039). The mode is chosen by extension unless `--std` says otherwise (see LANGUAGE.md, section 2).

Several inputs compile together: modules are ordered so each comes after the modules it imports, and the results are linked into one output. With `-c`, `--emit=asm`, `--emit=llvm-ir`, or `--emit=interface`, each input gets its own output; `-o` then needs a single input.

`crust -c x.crm -o dir/x.o` also writes `dir/<module>.crmi`. Same-package imports resolve against the inputs, then against `.crmi` files in the directory of `-o` (MODULES.md, section 7).

### Output

| Flag | Meaning |
|---|---|
| `-o <path>` | Output path |
| `--emit=<kind>` | `exe` (default), `obj`, `asm`, `llvm-ir`, `staticlib`, `sharedlib`, `header`, `interface` |
| `-c` | Shorthand for `--emit=obj` |
| `--emit-header=<path>` | Write a C header for `[[crust::c_abi]]` exports, alongside another output |

### Language and safety

| Flag | Meaning |
|---|---|
| `--std=c23` | C syntax. `let`, `mut`, `unsafe`, `import` are identifiers. Checks stay on. |
| `--std=crust` | Crust syntax, for any file extension |
| `--unchecked` | Turn off safety checks for every input. Warns on each use. For migration only. |

### Modules

| Flag | Meaning |
|---|---|
| `--extern <package>=<dir>` | Directory holding the `.crmi` interfaces of a dependency. Imports written `"<package>/name"` are found there. |
| `--print-interface <file>` | Print a `.crmi` file in readable form |

### Code generation

| Flag | Meaning |
|---|---|
| `-O0` … `-O3`, `-Os` | Optimization level. Default `-O0`. |
| `-g` | Debug info. Enables AddressSanitizer and LeakSanitizer when clang's sanitizer runtime is installed. |
| `--target=<triple>` | LLVM target triple. 0.1 supports `x86_64-pc-linux-gnu` only (D-044). |
| `-fPIC` | Position-independent code |

### Paths and linking

| Flag | Meaning |
|---|---|
| `-I <dir>`, `-isystem <dir>` | Include paths |
| `-D <name>[=value]`, `-U <name>` | Preprocessor macros |
| `-L <dir>`, `-l <name>` | Link paths and libraries |
| `-shared` | Link a shared library (same as `--emit=sharedlib`) |

### Compatibility

The common `cc` flags above are accepted with the same meaning. A build system that calls `cc -c foo.c -o foo.o` can call `crust -c foo.c -o foo.o`.

### Diagnostics

| Flag | Meaning |
|---|---|
| `--explain <CRnnnn>` | Print the long form of a diagnostic code |
| `--error-format=<human\|json>` | Output format. Default `human`. |
| `--version`, `--help` | |

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Compile error (diagnostics were emitted) |
| 2 | Usage error |
| 70 | Internal compiler error. Include the report in a bug report. |

## 2. `crepe`

```
crepe <command> [options]
```

| Command | Meaning |
|---|---|
| `crepe new <name>` | Create a package in a new directory |
| `crepe init` | Create a package in the current directory |
| `crepe add <name>[@<version>]` | Add a dependency. `--path <dir>` and `--git <url> [--rev <rev>]` are also accepted. |
| `crepe remove <name>` | Remove a dependency |
| `crepe build [--release] [--target <name>]` | Build the package and its dependencies, or only the target named in `crepe.toml` |
| `crepe run [--release] [--target <name>] [-- <args>]` | Build and run a binary (D-045) |
| `crepe test` | Build and run the `[[test]]` targets. Each passes if it exits 0. |
| `crepe check` | Type- and safety-check without generating code |
| `crepe fetch` | Download dependencies named in the lockfile |
| `crepe update [<name>]` | Re-resolve dependencies and rewrite the lockfile |
| `crepe tree` | Print the dependency tree |
| `crepe clean` | Remove `build/` |

Planned after M7: `crepe publish` and a registry.

### Manifest: `crepe.toml`

```toml
[package]
name = "example"
version = "0.1.0"

[dependencies]
mathx = "1.2"
parsekit = { git = "https://example.invalid/parsekit", rev = "a1b2c3d" }
local_util = { path = "../local_util" }
```

Dependencies use semantic versioning. Path and git dependencies work without a registry.

### Package layout

```
src/
  lib.crm           library root; declares `module <package>;`
  main.crm          binary root; declares `module <package>;`
  net/http.crm      any other module; declares `module net.http;`
  legacy/parse.cr   old-school source, paired with the header below
  legacy/parse.hr   Crust header included by parse.cr
```

### Build targets

Declared in `crepe.toml` as `[[bin]]`, `[[lib]]`, and `[[test]]` tables. Each names a `root` (a `.crm` module), or lists `sources` (old-school `.cr` files) with `include` directories for `.hr` headers. Modules are shared between targets. See [MODULES.md](MODULES.md), section 7.

Without target tables, `src/main.crm` is a binary and `src/lib.crm` a library, both named after the package. A `[[lib]]` builds `lib<name>.a` and the header `include/<name>.h`.

crepe calls `crust` from the same directory as `crepe`, or `$CRUST` if it is set, or `crust` on `PATH`.

### Lockfile: `crepe.lock`

Written by `crepe`, never edited by hand. Records the exact version or commit of each dependency. Commit it for binaries. Its format has no comments, so it is exempt from the file header rule (see CODE_STYLE.md).

### Output layout

```
build/
  debug/ or release/
    <package> executables and libraries
    include/       generated headers for C consumers
```

## 3. Environment variables

| Variable | Meaning |
|---|---|
| `CRUST_HOME` | Where `crepe` keeps its cache. Default: `~/.crepe`. |
| `CRUST_LLVM_CONFIG` | Path to `llvm-config`. crust uses the `clang` in its `--bindir`. Without it, crust runs `clang-18`, then `clang`, from `PATH`. |
| `CRUST` | The `crust` binary that crepe runs. |

## 4. Open questions

- Workspaces: several packages in one repository, sharing one lockfile. Not designed yet.
- Whether `crepe` supports editions, so the language can change without breaking old packages. Not decided (D-027).
- A registry (D-043), and packages with both `src/main.crm` and `src/lib.crm` (D-047).

<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Modules

Status: draft; implemented by Crust 0.1.0. **Accepted:** `.crm` modules are the modern way to organize Crust projects. `.cr` source with `.hr` headers stays for old-school code (D-020), and the details below (D-028 to D-036). Proposed refinements: D-039 and D-046. Related: [LANGUAGE.md](LANGUAGE.md), [ABI.md](ABI.md), [ARCHITECTURE.md](ARCHITECTURE.md), [CLI.md](CLI.md), [DECISIONS.md](DECISIONS.md).

## 1. Three kinds of Crust file

| Extension | What it is | Connects to other files by | Typical use |
|---|---|---|---|
| `.cr` | Crust source. Belongs to no module. | `#include "name.hr"`, `import` | Old-school source files, and source paired with a header |
| `.hr` | Crust header. Included, never compiled alone. | `#include` | Declarations shared by `.cr` files, the way a C header is |
| `.crm` | Crust module. Declares its own name at the top. | `import` and `export` | Modern projects: many files, several build targets, shared libraries |

Modules are the modern way to interlink Crust files. A module's interface is generated from its source, so each declaration exists once. Each module compiles once and is reused by every target that imports it.

A `.cr` and `.hr` pair is the old-school way. The header is written by hand and included as text. Macros in it reach every file that includes it, and each `.cr` file re-reads it. Pairs stay supported for existing code and for teams that want that style.

The module syntax follows TypeScript. The reserved words in every Crust file are `let`, `mut`, `unsafe`, `module`, `export`, `import`, and `ref` (D-012, D-018).

- `module` names the file's module. It comes first.
- `export` makes a declaration visible to importers.
- `import` brings exported names from another module into scope.

**Why modules.** A hand-written header repeats every declaration, so every declaration exists twice and the copies drift apart. Every file that includes it re-parses it, and macros and include order leak into every file. A module has one declaration, the compiler generates the interface from it, and nothing leaks.

**Choosing a form:**

| Situation | Use |
|---|---|
| One small program | `.cr` alone |
| A project with several files | `.crm` modules |
| More than one build target, or code shared by targets | `.crm`. Each module compiles once per configuration. |
| Existing `.cr` and `.hr` code | Keep it. `.cr` can import `.crm` modules as they are added, and a module can `#include` headers the team has not moved yet. |
| Shared code between packages | `.crm`, with `.crmi` interfaces through crepe |
| Callers in C | `.crm` modules with `[[crust::c_abi]]` exports. C sees the generated header. |
| C code | `.c` and `.h` (C23) |

## 2. A module at a glance

`src/geometry.crm`:

```c
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
module geometry;

#include <math.h>                       // C header, private to this module

export constexpr int MaxPoints = 1024;

export struct point {
    double x;
    double y;
};

double square(double v) {               // no `export`: private to this module
    return v * v;
}

export double distance(struct point a, struct point b) {
    return sqrt(square(a.x - b.x) + square(a.y - b.y));
}
```

`src/app.crm`:

```c
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
module app;

import { point, distance } from "geometry";
import * as geo from "geometry";

int main(void) {
    let a = (struct point){ .x = 0.0, .y = 0.0 };
    let b = (struct point){ .x = 3.0, .y = 4.0 };
    let limit = geo::MaxPoints;         // qualified through the namespace import
    if (distance(a, b) != 5.0 || limit != 1024) {
        return 1;
    }
    return 0;
}
```

Nothing in `app.crm` mentions a header. `square` cannot be used from `app.crm`, and an attempt is a compile error.

## 3. Declaring a module

- In a `.crm` file, the first declaration is `module <name>;`. Only the header comment may come before it.
- A `.cr` file declares no module. It has no name, cannot be imported, and is compiled into the targets that list it (section 7).
- Names are dotted. Each segment is snake_case: `geometry`, `net.http`, `net.http.client`.
- A module name is unique within a build. Two files that declare the same name are an error.
- By convention, `src/net/http.crm` declares `module net.http;`. A mismatch between path and name is a lint warning, not an error.
- The root modules are named after their package. A library's root is `src/lib.crm`. A binary's root is `src/main.crm`. Both declare `module <package>;` (see [CLI.md](CLI.md)).
- Order inside a `.crm` file: header comment, `module`, `#include` lines, `import` lines, then declarations.

## 4. Exporting

```c
export int add(int a, int b);                 // 1. export a declaration
export { add, sub };                          // 2. export names declared in this file
export { add as plus } from "mathx.core";     // 3. re-export a name from another module
export * from "mathx.core";                   // 4. re-export everything it exports
```

- Functions, variables, `constexpr` constants, structs, unions, enums, and typedefs can be exported.
- A declaration without `export` is private. Private declarations never appear in the interface file.
- There are no default exports in v1 (D-033).
- An export is a promise. Changing an exported name or signature changes the interface, so importers rebuild. If the export is `[[crust::c_abi]]`, the change is also an ABI break (section 10).

## 5. Importing

```c
import { add, sub as subtract } from "mathx.core";   // named imports
import * as core from "mathx.core";                  // namespace import
let total = core::add(1, 2);
```

- `"name"` names a module in the same package. `"package/name"` names a module in a dependency. A package's root module is named by the package alone, so `"mathx"` imports the root of `mathx`.
- Only packages listed under `[dependencies]` in `crepe.toml` can be imported (D-029). Any other package is an error.
- Only exported names can be imported. Importing a private name is an error, and the message lists the exports.
- `::` qualifies names through a namespace import. In valid C it appears only inside `[[...]]` attributes, so it does not conflict with C code.
- An imported name refers to every export of that name, in every namespace. A struct is a tag, so `import { point }` lets you write `struct point`, as in section 2.
- Macros are never imported (section 8).
- Two imports of the same name, or an import and a local declaration of the same name, is an error at the second one.
- Exporting an enum exports its enumerators too, as constants (D-046). Import them by name, like any other export, or reach them through a namespace import: `case colors::ColorRed:`.
- A `.cr` file may import modules under the same rules. A `.cr` file cannot export, because it has no module name.

## 6. Interfaces: the header replacement

When the compiler compiles a `.crm` module, it also writes a module interface file, `.crmi`. The interface holds:

- every export, with its kind, type, layout, signature (including lifetimes), and attributes
- the types and drop functions those exports need
- a hash of the interface

Importers read the `.crmi` file, not the source. The compiler generates it, so nothing is written twice and nothing drifts. This plays the role of a TypeScript `.d.ts` file, except the compiler always produces it.

**Rebuild rule (D-030).** An importer rebuilds only when an imported module's interface hash changes. Editing a function body does not rebuild importers. This is the only incremental build in v1 (see [ARCHITECTURE.md](ARCHITECTURE.md), section 9).

Interface files are binary and versioned. `crust --emit=interface` writes one. `crust --print-interface <file>` prints a readable form for review.

## 7. Building modules and targets

- crepe finds the `.crm` modules in a package by scanning `src/`, and reads the build targets from `crepe.toml`. It builds the module graph from the imports.
- crust compiles one module at a time, in dependency order. Each module produces an object file and a `.crmi` file. `crust -c src/x.crm -o dir/x.o` writes `dir/<module>.crmi` next to the object, and rewrites it only if it changed (D-039).
- crust resolves a same-package import `"name"` against the modules given in the same invocation, then against `<module>.crmi` in the directory of `-o`. A dependency import `"package/name"` reads `<dir>/<name>.crmi` from `--extern package=<dir>`, and `"package"` reads `<dir>/<package>.crmi`.
- **No import cycles (D-031).** A cycle is an error in v1. Move the shared declarations into a third module.
- **Module-level initializers are constant (D-031).** Every module-level variable, including `constexpr` and `let`, takes a constant initializer. Nothing runs at startup on behalf of a module, so importing a module never runs code and there is no initialization order to get wrong.
- **Module-level `let mut` needs `unsafe`.** Mutable module state can be declared. Reading or writing it requires `unsafe` until thread-safety rules land (M8).
- **`.cr` files have no interface file.** A change to a `.cr` file rebuilds every target that compiles it. A change to a `.crm` module rebuilds its importers only when its interface changes (D-030).

### Build targets (D-036)

A package declares its build targets in `crepe.toml`. A target is an executable, a library, or a test. Targets share modules:

```toml
[[bin]]
name = "app"
root = "src/main.crm"

[[lib]]
name = "geometry"
root = "src/geometry.crm"

[[bin]]
name = "legacy-tool"
sources = ["src/tool/main.cr", "src/tool/parse.cr"]
include = ["src/tool"]              # directories searched for "name.hr" headers
```

- A `.crm` module is compiled once per configuration (debug or release, target triple) and linked into every target that imports it.
- A target built from `.cr` sources lists them explicitly. `include` gives the directories searched for `.hr` headers.
- crepe builds only the targets asked for, for example `crepe build --target app`. Targets that import the same module reuse its build.

## 8. Crust, C, and old-school headers

| You want to | Do this |
|---|---|
| Use another Crust module | `import` it, from a `.crm` or `.cr` file |
| Use an old-school Crust header | `#include "name.hr"` from a `.cr` or `.crm` file |
| Use a C library | `#include` its header from a `.cr` or `.crm` file. Its declarations stay inside that file. |
| Use a Crust library from C | `#include` the header that `crust --emit=header` generates. It declares the `[[crust::c_abi]]` exports. |
| Use a dependency package | List it in `crepe.toml`, then `import ... from "package/name"` |

Rules (D-032, D-035):

- `.c` and `.h` files are C. They cannot `import` and do not see `.crm` exports. C uses the generated headers.
- `#include` of a `.crm` file is an error. Use `import`.
- `.hr` files are Crust, parsed in Crust mode. `.h` files included from Crust are parsed as C, so `export`, `module`, and `import` stay ordinary identifiers inside C headers.
- `#include` is textual. Macros in a `.hr` or `.h` file reach the file that includes it. This is the old-school behavior. Macros in a `.crm` module never leave it.
- A `.cr` definition must match its `.hr` declaration. The compiler checks each definition's signature against the header and reports a mismatch (D-035).
- C declarations from `#include` are not re-exported from a `.crm` module. To make a C function part of a module's API, write an exported wrapper.

## 9. Safety at module boundaries

A module is the unit that owns its invariants.

- **Privacy is a safety boundary.** Private code and `unsafe` blocks can rely on invariants that only the module maintains. A safe export must not let callers break them. The checker does not prove this. A module's safe exports are a contract the module's authors review, and a violation is a soundness bug (see [MEMORY_SAFETY.md](MEMORY_SAFETY.md), sections 2 and 9).
- **Pairs have no privacy.** Every declaration in a `.hr` header is visible to every file that includes it, and a `.cr` file has no export list. An invariant that must stay inside a set of files belongs in a `.crm` module.
- **Owned types.** Exporting an owned type exports its drop function with it. An importer that owns a value of the type releases it through that function.
- **Lifetimes.** Lifetimes in exported signatures follow the usual rules: elided where unambiguous, written as `'a` otherwise. The interface records them, so importers check borrows against the same signatures.
- **Unsafe exports.** A function declared `unsafe` is unsafe to call from any module (see [LANGUAGE.md](LANGUAGE.md), section 9).

## 10. Modules and the C ABI

Exports come in two kinds (D-034, which amends D-003):

| Export | Callable from | Allowed types | Symbol | In generated header |
|---|---|---|---|---|
| `export` | Crust modules | Any Crust type | Mangled by module name | No |
| `[[crust::c_abi]] export` | Crust and C | C-compatible types only | Plain C name | Yes |

```c
[[crust::c_abi]] export int crust_add(int a, int b) {
    return a + b;
}
```

`crust --emit=header` (or `--emit-header=<path>` next to another output) writes a C23 header with the `[[crust::c_abi]]` exports and the struct and enum definitions their signatures need. crepe writes it to `build/<profile>/include/<lib>.h` for a `[[lib]]` target.

See [ABI.md](ABI.md) for the layout and calling rules.

## 11. Scope, compared with TypeScript

| TypeScript feature | In Crust v1 |
|---|---|
| Named `export` and `import` | Yes |
| `export { x as y }` and `export { x } from "m"` | Yes |
| `export * from "m"` | Yes |
| `import * as ns from "m"` | Yes |
| `export default` and default imports | No (D-033) |
| `import type` | No. Types are imported like any other name. |
| Dynamic `import()` | No. Imports are resolved at build time. |
| Side-effect `import "m";` | No. Importing runs no code (section 7). |
| Circular imports | No (D-031) |
| `export` in a `.cr` file | No. A `.cr` file has no module name. Use `.crm`. |

Open questions about modules are listed under D-028 to D-036 in [DECISIONS.md](DECISIONS.md).

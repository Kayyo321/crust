<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Crust

The C compiler with guaranteed memory safety, by replicating the Rust language's rules.

Crust compiles C23 (ISO/IEC 9899:2024) plus a small set of additions (`let`, lifetimes, `unsafe`, and `.crm` modules) through LLVM into binaries and libraries. Checked code cannot leak memory, use freed memory, double-free, or overflow a buffer. Full C ABI compatibility means existing C code links and runs unchanged. Crust projects are organized in modules (`.crm`), which need no header files. Older source and header pairs (`.cr` and `.hr`) are supported too.

**Status: 0.1.0, pre-alpha.** The compiler and the package manager work for x86-64 Linux. The language and flags can still change before 1.0.

| Tool | Purpose |
|---|---|
| `crust` | Compiler: C23 + Crust to binaries and libraries |
| `crepe` | Package manager for Crust libraries, like cargo |

## Quick start

You need Rust 1.85 or newer and LLVM 18's `clang`.

```bash
cargo build --workspace
```

```bash
target/debug/crepe new hello && cd hello && ../target/debug/crepe run
```

```bash
target/debug/crust hello.c -o hello
```

## Example

`src/app.crm` imports from `src/geometry.crm`, which exports `point` and `distance`. The example needs no header file.

```c
module app;

import { point, distance } from "geometry";

int main(void) {
    let a = (struct point){ .x = 0.0, .y = 0.0 };   // immutable binding
    let b = (struct point){ .x = 3.0, .y = 4.0 };
    return distance(a, b) == 5.0 ? 0 : 1;
}
```

## Documentation

- [PROJECT.md](PROJECT.md): goals, scope, milestones
- [docs/README.md](docs/README.md): index of the full documentation set
- [docs/MODULES.md](docs/MODULES.md): modules, imports, and exports, which replace headers
- [AGENTS.md](AGENTS.md) and [CLAUDE.md](CLAUDE.md): rules for AI coding agents
- [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md): how to contribute

## License

Apache License 2.0. See [LICENSE](LICENSE).

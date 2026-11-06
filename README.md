<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Crust

The C compiler with guaranteed memory safety, by replicating the Rust language's rules.

Crust compiles C23 (ISO/IEC 9899:2024) plus a small set of additions (`let`, lifetimes, `unsafe`, and `.crm` modules) through LLVM into binaries and libraries. Checked code cannot leak memory, use freed memory, double-free, or overflow a buffer. Full C ABI compatibility means existing C code links and runs unchanged. Crust projects are organized in modules (`.crm`), which need no header files. Older source and header pairs (`.cr` and `.hr`) are supported too.

**Status: pre-alpha.** This repository holds design documents only. There is no compiler yet.

| Tool | Purpose |
|---|---|
| `crust` | Compiler: C23 + Crust to binaries and libraries |
| `crepe` | Package manager for Crust libraries, like cargo |

## Example (illustrative syntax, not final)

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

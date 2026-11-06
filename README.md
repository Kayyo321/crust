<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Crust

The C compiler with guaranteed memory safety, by replicating the Rust language's rules.

Crust compiles C23 (ISO/IEC 9899:2024) plus a small set of additions (`let`, lifetimes, `unsafe`) through LLVM into binaries and libraries. Checked code cannot leak memory, use freed memory, double-free, or overflow a buffer. Full C ABI compatibility means existing C code links and runs unchanged.

**Status: pre-alpha.** This repository holds design documents only. There is no compiler yet.

| Tool | Purpose |
|---|---|
| `crust` | Compiler: C23 + Crust to binaries and libraries |
| `crepe` | Package manager for Crust libraries, like cargo |

## Example (illustrative syntax, not final)

```c
#include <stdio.h>

int main(void) {
    let greeting = "hello, crust";   // immutable binding, type inferred
    printf("%s\n", greeting);
    return 0;
}
```

## Documentation

- [PROJECT.md](PROJECT.md): goals, scope, milestones
- [docs/README.md](docs/README.md): index of the full documentation set
- [AGENTS.md](AGENTS.md) and [CLAUDE.md](CLAUDE.md): rules for AI coding agents
- [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md): how to contribute

## License

Apache License 2.0. See [LICENSE](LICENSE).

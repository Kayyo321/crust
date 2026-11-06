<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Code style

Status: binding for all code and docs in this repo. The naming and header rules are project rules. Formatting is enforced by tools. Related: [CONTRIBUTING.md](CONTRIBUTING.md), [AGENTS.md](../AGENTS.md).

## 1. File header

Every file starts with the signature, exactly as below. Do not change the text, the line breaks, or the spacing.

**C and Crust source (`.c`, `.h`, `.cr`, `.hr`, `.crm`), and other C-family source:**

```c
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
```

**Markdown:** the same block, wrapped in an HTML comment so it does not render:

```markdown
<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->
```

**Files whose comment syntax is `#` (shell, TOML, YAML, `.gitignore`):**

```
# Ko, (Baczynski-Bruce) 2026; Apache 2.0
```

**Exempt files:** `LICENSE` (license text stays verbatim), `crepe.lock` and other machine-written files with no comment syntax (JSON, binary, images). Generated source files carry the header too. Generators emit it from their template.

Formatters must not rewrite the header. If a formatter changes it, wrap it in `// clang-format off` and `// clang-format on`, or mark the region the way the formatter expects.

## 2. Naming

The house rule: **`snake_case` everywhere, except constants and enums, which are `PascalCase`. Never `camelCase`.**

| Thing | Style | Example |
|---|---|---|
| Functions | snake_case | `parse_expr` |
| Variables, parameters, struct fields | snake_case | `token_count` |
| Structs, unions, typedefs | snake_case | `token_stream` |
| Enum types | PascalCase | `TokenKind` |
| Enumerators | PascalCase, prefixed with the enum name | `TokenKindIdent` |
| Constants (`constexpr`, object-like macros) | PascalCase | `MaxDepth` |
| Labels | snake_case | `cleanup` |
| Crust lifetimes | snake_case, with the leading apostrophe | `'a`, `'buf` |
| Crust attributes | snake_case | `[[crust::drop]]` |
| Test functions | snake_case, describing the expected result | `rejects_double_free` |
| Code file and directory names | snake_case | `borrow_check.c` |
| Module names | dotted; each segment snake_case | `net.http` |
| Module file paths | match the module name, dots become slashes | `src/net/http.crm` |

Rules that follow from the house rule:

1. **PascalCase is for constants and enums only.** A PascalCase struct or function is a style error.
2. **Enumerators carry the enum name.** C enumerators share one namespace, so `TokenKind` gets `TokenKindIdent`. Shorter names collide.
3. **Acronyms are words.** `Llvm`, not `LLVM`, in PascalCase: `IrKindLlvm`. In snake_case: `llvm_ir`.
4. **No leading underscore.** C reserves identifiers that begin with `_` followed by an uppercase letter, or that contain `__`. Don't use them.
5. **Prefer `constexpr` over `#define`** for constants. An object-like `#define` that must stay a macro is still PascalCase.
6. **Documentation files are the exception.** Project-level docs use the conventional uppercase names that GitHub recognizes: `README.md`, `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `AGENTS.md`, `CLAUDE.md`, `PROJECT.md`, and `docs/*.md` in capitals. These names are not code.

## 3. Formatting

- Indent with 4 spaces. No tabs.
- Maximum line length: 100 columns.
- Braces: K&R. Opening brace on the same line. `else` on the closing brace's line.
- Always use braces for `if`, `else`, `for`, `while`, and `do`, even for one statement.
- One statement per line. One declaration per line.
- The formatter is the source of truth. Don't hand-format against it.
  - C and headers: `clang-format` with the checked-in `.clang-format` (added in M0).
  - `.cr`, `.hr`, and `.crm` files: formatter TBD. Planned as `crust fmt`. Until then, use `clang-format` only if it produces the same result on your file.
  - Rust implementation code, if chosen (D-019): `rustfmt` with the checked-in `rustfmt.toml`.

## 4. Comments

- `//` for short comments inside functions. `/** */` for doc comments on public items.
- Explain **why**, not what. The code says what. A comment that restates the code is noise.
- No commented-out code. Delete it. Git keeps it.
- `TODO` and `FIXME` must reference an issue: `TODO(#123): ...`. A bare `TODO` fails review.
- Every `unsafe` block, in Crust code or in the compiler's own code, has a `SAFETY:` comment directly above it. It says which invariant holds and why.

## 5. Errors and diagnostics

- User errors are diagnostics. They go through the diagnostics API (`diag/`) and carry a code (see ARCHITECTURE.md). Never print directly from a pass.
- Only the driver exits the process. Passes return results or diagnostics.
- An internal compiler error (a bug in Crust, not in the user's code) reports through the ICE path. It prints a bug-report message, includes the input, and exits with code 70. It never continues silently.
- Don't `assert` on user input. Asserts are for invariants inside the compiler.

## 6. Functions and files

- A function that does not fit on one screen (about 60 lines) should be split. This is a guideline. A reviewer can ask for the split.
- A file should have one main concern. If a file grows past about 1000 lines, consider splitting it.
- Public API is marked `export` in a `.crm` module, or declared in a `.hr` header for a source/header pair. Internals stay private (`static` in C). A `.crm` file is one module, so it has one main concern by definition.

## 7. Language-specific rules

### C (compiler sources, if C is chosen)

- Use C23 features where they help: `bool`, `nullptr`, `constexpr`, `static_assert` without a message, and `auto` for initializers whose type is obvious.
- Mark file-local functions `static`.
- Use `const` for every parameter and variable that is not modified.

### Rust (compiler sources, if Rust is chosen; D-019)

The house naming rule conflicts with rustc's default lints. The crate root must allow them:

```rust
#![allow(non_camel_case_types)]     // structs and typedefs are snake_case
#![allow(non_upper_case_globals)]   // constants are PascalCase
```

Also required: `#![deny(unsafe_op_in_unsafe_fn)]`, and the Clippy lint `undocumented_unsafe_blocks` set to `deny`.

### Crust (`.cr`, `.hr`, `.crm` files, tests, and `lib/`)

- Prefer `let` without a type annotation when the type is obvious from the initializer.
- Write lifetime annotations only where elision fails.
- Attributes are lowercase: `[[crust::drop]]`, not `[[crust::Drop]]`.
- Order inside a `.crm` module: `module`, then `#include`, then `import`, then declarations. In a `.cr` file, `#include` the `.hr` header before any code. Never `#include` a `.crm` file. Import it.
- Export only what callers need. Private is the default. See [MODULES.md](MODULES.md).
- Use `mut` only where mutation is needed. Immutability is the default.

## 8. Enforcement

- **Now (M0 target):** a `tools/check_style` script checks the header on every file, flags identifiers that break the naming rule, and runs `clang-format --dry-run` on C sources. It runs in CI.
- **Review:** everything the script can't check, reviewers check. Naming in new code is in scope for review.
- **Exceptions:** an exception to a rule needs a comment that names the rule and says why. Exceptions to the header rule need a DECISIONS.md entry.

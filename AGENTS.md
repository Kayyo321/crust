<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# AGENTS.md

Instructions for AI coding agents working in this repository. Human contributors: see [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

## What this project is

Crust is a C compiler for C23 with a checked mode that enforces Rust's memory-safety rules: ownership, borrowing, lifetimes, definite initialization, and drop checking. It emits code through LLVM. It ships two CLIs: `crust` (the compiler) and `crepe` (the package manager). Read [PROJECT.md](PROJECT.md) first.

## Current state

Design documents only. There is no build system yet. When one lands (milestone M0 in PROJECT.md), replace the placeholder commands in docs/TESTING.md with real ones. Do not invent build or test commands.

## Where things are

- [PROJECT.md](PROJECT.md): goals, non-goals, milestones, glossary
- [docs/LANGUAGE.md](docs/LANGUAGE.md): what the Crust language is
- [docs/MEMORY_SAFETY.md](docs/MEMORY_SAFETY.md): the guarantees, their limits, and how they are enforced
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): compiler pipeline and repo layout
- [docs/ABI.md](docs/ABI.md): C ABI rules
- [docs/CLI.md](docs/CLI.md): `crust` and `crepe` commands
- [docs/CODE_STYLE.md](docs/CODE_STYLE.md): naming, formatting, file header. Read before writing code.
- [docs/TESTING.md](docs/TESTING.md): which tests each change needs
- [docs/DECISIONS.md](docs/DECISIONS.md): accepted, proposed, and open design decisions

## Hard rules

1. **Every file starts with the signature header** in docs/CODE_STYLE.md. Do not omit it or change its text. The only exception is `LICENSE`, which stays verbatim.
2. **Naming:** `snake_case` for everything, except constants and enums, which are `PascalCase`. Never `camelCase`. Enumerators are prefixed with their enum name, for example `TokenKindIdent`.
3. **Do not add language syntax, keywords, attributes, or flags** that docs/LANGUAGE.md, docs/CLI.md, or docs/DECISIONS.md do not already define. Propose them as a DECISIONS.md entry first.
4. **Never weaken a safety rule to make a test pass.** If a test conflicts with a guarantee in docs/MEMORY_SAFETY.md, stop and report the conflict.
5. **Every `unsafe` block needs a `SAFETY:` comment** that states why the code upholds its invariants.
6. **Update the docs with the behavior.** The docs are the spec. If you change behavior, change the doc in the same commit.
7. **Add the tests that docs/TESTING.md requires** for your change. Do not lower a coverage floor.
8. **Justify new dependencies** in the PR description.
9. **Keep commits to one concern.**

## Git

- Push to the branch you were assigned when a task is complete.
- Push to `main` only when explicitly asked.
- Do not create other branches unless asked.
- Do not open pull requests unless asked.
- Commit message format is in docs/CONTRIBUTING.md.

## When unsure

Say what is unclear. Do not guess language semantics. If a question is listed as open in docs/DECISIONS.md, treat it as undecided and ask before implementing it.

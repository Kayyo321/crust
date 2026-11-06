<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Crust documentation

Start with [PROJECT.md](../PROJECT.md) for the goals, then read what applies to your work.

## If you are reading the language

1. [LANGUAGE.md](LANGUAGE.md): what Crust is, its modes, and its additions to C23
2. [MODULES.md](MODULES.md): modules, imports, and exports, the modern way to organize Crust projects, and how they coexist with `.cr` and `.hr` pairs
3. [MEMORY_SAFETY.md](MEMORY_SAFETY.md): the guarantees, their limits, and how they are enforced
4. [ABI.md](ABI.md): how Crust and C interoperate

## If you are building the compiler or tools

1. [ARCHITECTURE.md](ARCHITECTURE.md): the pipeline and repo layout
2. [CLI.md](CLI.md): `crust` and `crepe` commands and flags
3. [DECISIONS.md](DECISIONS.md): what is decided, and what is still open. Check this before implementing anything new.

## If you are contributing

1. [CONTRIBUTING.md](CONTRIBUTING.md): how changes get made and merged
2. [CODE_STYLE.md](CODE_STYLE.md): file header, naming, formatting. Checked by tools and review.
3. [TESTING.md](TESTING.md): which tests each kind of change needs
4. [SECURITY.md](SECURITY.md): how to report a soundness or security bug

## If you are an AI agent

[AGENTS.md](../AGENTS.md) has the rules for every agent. [CLAUDE.md](../CLAUDE.md) adds Claude Code notes.

## Document status

Every document is a draft until the milestone it depends on lands. A document's **Status** line says which. When behavior changes, update the document in the same change.

<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Changelog

Newest first. Each release has a `## MAJOR.MINOR.PATCH` heading, and `tests/version.rs` checks that the current version has one. The conformance pass rate and the failing C programs belong here once the conformance suite exists ([TESTING.md](docs/TESTING.md), section 8).

## 0.1.1

A testing release. The language, the ABI, the CLIs, and the safety rules are unchanged, so everything the docs say is limited in 0.1 still is.

- **`ensure`**: a fluent assertion library for Rust tests, so a check reads as a sentence: `ensure(code).named("exit code").is(0)`, `ensure(stderr).cannot_contain("panicked")`, `ensure(found).is_some().is("geometry")`. A failure shows what was wanted and what was found, and points at the line of the test. See [TESTING.md](docs/TESTING.md), section 9.
- The integration tests in `tests/` and `crepe/tests/` are written with `ensure`.
- `tests/version.rs` keeps `crust --version`, the README status line, and this file in step with the workspace version.
- Messages that named "Crust 0.1.0" as the limit of a feature (unsupported features, `CR1301` and friends, the target error, and crepe's registry error) now say "Crust 0.1", because the limit belongs to the release line.

## 0.1.0

The first release: the `crust` compiler and the `crepe` package manager for x86-64 Linux, `.crm` modules, and the first part of the safety core. See [PROJECT.md](PROJECT.md) for what it covers.

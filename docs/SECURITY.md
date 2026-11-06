<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Security policy

## Supported versions

Crust is pre-alpha. There are no releases yet. Only `main` is supported.

## What counts as a security issue

Crust's job is to make memory errors impossible in checked code, so the bugs that matter most are:

- **Soundness bugs.** The checker accepts checked code that leaks, double-frees, uses freed memory, reads uninitialized memory, overflows a buffer, or mutates through a live shared borrow.
- **Miscompilation.** The compiler produces code that violates a guarantee, even though the source was accepted.
- **ABI bugs** that corrupt memory across the C boundary.
- **Module-boundary bugs.** A safe export that lets callers break an invariant the module relies on. See [MODULES.md](MODULES.md), section 9.
- **Compiler crashes** on untrusted input, when the crash can be turned into memory corruption.

Ordinary bugs (a wrong diagnostic message, a missing feature) go in a public issue.

## Reporting

Report security issues privately, through GitHub's private vulnerability reporting on this repository (the Security tab, then "Report a vulnerability"). Don't open a public issue or pull request for a soundness bug until a fix is available.

Include, if you can:

- The smallest program that shows the problem
- The compiler flags used
- What the checker said, and what the program actually did

## Response

This is a small project. We aim to acknowledge reports within a week, and to agree on a fix plan with you. Credit is given in the fix if you want it.

## Fixing

A soundness fix adds a case to the soundness corpus before the fix lands, so the bug can't come back silently (see [TESTING.md](TESTING.md), section 7). The known-bugs list in [DECISIONS.md](DECISIONS.md) records the bug and its status.

<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Contributing to Crust

Thanks for helping. Crust makes a strong promise, memory safety for checked code, and we keep it by being careful about what we accept. Careful is not slow: small, well-tested changes get merged quickly.

## Before you start

Read, in this order:

1. [PROJECT.md](../PROJECT.md): what we are building and what we are not
2. [CODE_STYLE.md](CODE_STYLE.md): the file header and naming rules, which are checked
3. [DECISIONS.md](DECISIONS.md): what is decided, and what is still open

If your change touches the language, the ABI, or a safety rule, also read [LANGUAGE.md](LANGUAGE.md), [MEMORY_SAFETY.md](MEMORY_SAFETY.md), and [ABI.md](ABI.md). If it touches modules, imports, exports, headers, or interface files, read [MODULES.md](MODULES.md).

## Setup

Not yet available. Setup steps arrive with milestone M0 and are written here then. Until then, there is nothing to build. You can still improve the design docs.

## What kind of change

| Change | How to propose it |
|---|---|
| Bug fix, refactor, docs, tests | Pull request. No discussion needed first. |
| New diagnostic or error message | Pull request. |
| New language syntax, keyword, attribute, module rule, or CLI flag | A `Proposed` entry in [DECISIONS.md](DECISIONS.md), then discussion, then `Accepted` before code. |
| Change to a safety rule | A DECISIONS.md entry with the reason. A rule is never weakened silently. |
| ABI change | A DECISIONS.md entry, and a note in the PR. |
| Security or soundness bug | Report privately first. See [SECURITY.md](SECURITY.md). |

## Workflow

We use trunk-based development. `main` should always build and pass the PR gate.

- Keep branches short-lived. Merge within a few days.
- Maintainers may push small, green changes straight to `main`. Anything that needs review goes through a pull request.
- Rebase on `main` before merging. Keep history linear.

## Commits

Subject line: `area: imperative summary`, at most 72 characters.

```
sema: reject switch over enum with missing enumerator

Checked code must cover every enumerator or include default. Before
this change, a missing case compiled and fell through at run time.

Fixes #123
```

Body: explain **why**. The diff already says what changed. One concern per commit.

## Pull requests

Use the template in `.github/PULL_REQUEST_TEMPLATE.md`. It asks for:

- what and why
- the change type (bug fix, language or ABI change, soundness-affecting, refactor, docs)
- the tests that [TESTING.md](TESTING.md) requires for that change type
- docs updated, if behavior changed

Keep PRs small. A PR that can be reviewed in one sitting is ideal. If it can't, split it.

## Review

- One approving review from a maintainer for ordinary changes.
- Soundness-affecting and ABI-affecting changes say so in the PR. They get a closer read, and a soundness case must be in the PR.
- Review checks: correctness, tests that match the change, naming and header rules, and docs.

## Definition of done

A change is done when:

- [ ] It does what the PR says, and the tests that TESTING.md requires are in it.
- [ ] Every new file has the header from CODE_STYLE.md. Naming follows the rules.
- [ ] The formatter and the style check pass.
- [ ] Docs match the new behavior.
- [ ] No coverage floor was lowered.
- [ ] Any new language, ABI, or safety behavior has a DECISIONS.md entry.

## Dependencies

Adding a dependency needs a reason in the PR. Prefer what the platform already has. A dependency that handles untrusted input or is on the checked path needs a DECISIONS.md entry.

## Conduct

Be specific and kind in reviews. Critique the code, not the person. If a discussion stalls, say what is blocking it and what would settle it.

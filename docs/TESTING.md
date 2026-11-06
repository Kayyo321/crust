<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# Testing

Status: draft. Principles: tests prove the guarantees, feedback stays fast, and coverage is a floor, not a score to chase. Related: [MEMORY_SAFETY.md](MEMORY_SAFETY.md), [CONTRIBUTING.md](CONTRIBUTING.md).

## 1. Commands

Run from the repository root. Prerequisites are in [CONTRIBUTING.md](CONTRIBUTING.md).

| Task | Command |
|---|---|
| Build `crust` and `crepe` | `cargo build --workspace` |
| Fast tests (unit tests only) | `cargo test --workspace --lib --bins` |
| Full tests (the PR gate) | `cargo test --workspace` |
| One suite | `cargo test --test suite`, `--test abi`, `--test interface`, `--test examples` |
| The `ensure` library | `cargo test -p ensure` |
| Style check | `tools/check_style` |
| Formatting | `cargo fmt --all --check` |
| Lints | `cargo clippy --all-targets --workspace -- -D warnings` |
| Update the golden interface | `CRUST_BLESS=1 cargo test --test interface`, then review the diff |

The suite runs every run-pass program under valgrind's leak checker when `/usr/bin/valgrind` exists. Set `CRUST_TEST_VALGRIND=0` to skip it. Coverage is not measured yet; the floors in section 4 start at M2.

## 2. Test kinds

| Kind | Location | Checks | Runs on |
|---|---|---|---|
| Unit | next to the code | One pass or function | Every PR |
| Compile-fail | `tests/compile_fail/` | Each rule rejects its bad programs with the right code | Every PR |
| Run-pass | `tests/run_pass/` | Programs compile and run with expected stdout and exit code, under ASan and LSan | Every PR |
| Module and pair | `tests/modules/`, `tests/pairs/` | Multi-module programs build and run. Import and export errors reject with the right code. `.cr` and `.hr` pairs compile, and mismatched signatures reject. `.crmi` golden files match. | Every PR |
| Soundness corpus | `tests/soundness/` | Must-reject and must-accept cases, each tagged with a guarantee ID (G-UAF, G-DF, G-LEAK, G-OOB, G-UNINIT, G-ALIAS) | Every PR |
| C conformance | `tests/conformance/` | A C23 conformance suite in C mode. Pass rate may only go up. | Nightly |
| ABI matrix | `tests/abi/` | Layout and call tests in both directions, per target | Nightly and release |
| Fuzzing | `fuzz/` | Lexer, parser, and checker never crash or hang | Nightly, time-boxed |
| Full sanitizer run | all run-pass | ASan, UBSan, and LSan on everything | Nightly |

The runner is `tests/suite.rs`. A test case is a file, or a directory whose `.c`, `.cr`, and `.crm` files compile together. ABI tests are in `tests/abi.rs`; interface golden and rebuild tests are in `tests/interface.rs`.

Compile-fail tests mark the line that should be flagged with a comment, and name the code:

```c
let mut a = owned_bytes(8);
let b = a;
a[0] = 1;   //~ ERROR CR0382
```

Each test file starts with the file header from CODE_STYLE.md. Expected output lives beside the input, with the same name and a `.stdout` extension (`expect.stdout` in a directory case). A `// args: ...` line passes extra flags to crust. Soundness cases name their guarantee with a `// guarantee: G-...` line.

Test names are `snake_case` and describe the expected result: `rejects_use_after_move`, `accepts_reborrow_after_last_use`.

## 3. What each change needs

This table is the whole policy. If a change is not listed, ask in the PR.

| Change | Required |
|---|---|
| New language rule | A compile-fail test that the rule rejects, and a run-pass test of the valid variant that it accepts |
| New diagnostic code | A compile-fail test that triggers it, matched by code |
| New module rule (import, export, naming, cycles) | A compile-fail test that the rule rejects, and a multi-module run-pass test that it accepts |
| Source/header pair rule | A compile-fail test with a mismatched signature, and a run-pass pair that matches |
| Interface format or hash change | A golden `.crmi` test, updated in the same PR |
| Function body change only | A test that the interface hash is unchanged and importers are not rebuilt |
| Bug fix, not a soundness bug | A regression test that fails before the fix and passes after |
| Soundness fix | A soundness corpus case, added before the fix. It fails on `main`. |
| New ABI-visible type or function | An ABI layout test, and a call test |
| Refactor, no behavior change | Existing tests pass. No new tests required. |
| Performance change | Benchmark numbers in the PR description |
| Docs only | None |

Keep the tests that a change needs. Don't add tests to pad coverage.

## 4. Coverage

- Coverage is measured on every PR and reported in the PR.
- **Floors** (gates):
  - Overall line coverage: 80%.
  - `src/safety/` (the analyses): 90% line coverage and 85% branch coverage.
- **Ratchet:** a PR may not reduce overall coverage by more than 0.5 points.
- Floors are measured from M2 onward. Before M2, coverage is reported only.
- Code that cannot be reached gets a comment naming the reason. Reviewers check those comments. A lot of them is a smell.
- Don't chase 100%. A missed branch that the soundness corpus already exercises is fine.

## 5. Speed

- The PR gate runs in under 10 minutes on CI. If it grows past that, move slow tests to nightly. Don't skip tests.
- Unit tests run in under a minute on a developer machine. A slower unit test needs a reason.

## 6. Flaky tests

A flaky test is a bug.

1. Remove it from the PR gate within one working day, and open an issue.
2. Fix it or delete it within a week.
3. Don't re-run CI until it passes. A test that passes on retry hides the problem.

## 7. Soundness testing

- Each guarantee in MEMORY_SAFETY.md section 1 has corpus cases that must reject, and cases that must accept. Accept-cases stop over-rejection. Without them, the checker can be sound and useless.
- Run-pass tests run under LeakSanitizer. A leak in a checked program is a test failure, which tests G-LEAK on real code.
- Fuzzing targets the checker, not only the parser. A checker crash on valid input is a bug. Checker acceptance of invalid input is a soundness bug.

## 8. Reporting

- CI publishes the test summary, the coverage report, and the conformance pass rate on each run.
- Conformance pass rate and the list of failing C programs are tracked in each release note.

## 9. Writing checks with `ensure`

Rust tests in this repo use the `ensure` crate (`ensure/`, a workspace member and a dev-dependency of `crust` and `crepe`). A check reads as a sentence: `ensure(value)`, then what must hold.

```rust
use ensure::*;

ensure(exit_code).named("exit code").is(0);
ensure(stderr).cannot_contain("panicked");
ensure(lines).has_len(3).contains("Compiling util");
ensure(found).is_some().is("geometry");
ensure(|| divide(1, 0)).panics().contains("zero");
ensure(Command::new(crust).output().unwrap()).succeeds().stdout().contains("crust");
```

A failed check panics at the line of the test, and shows what it wanted and what it found:

```
ensure failed: exit code
  wanted: exactly 0
   found: 3
```

| Kind | Checks |
|---|---|
| Equality | `is(x)`, `cannot_be(x)`, `is_one_of([..])`, `cannot_be_one_of([..])`, `satisfies("is even", \|n\| n % 2 == 0)` |
| Order | `is_above(x)`, `is_below(x)`, `is_at_least(x)`, `is_at_most(x)`, `is_between(low, high)`, `is_close_to(x, tolerance)` for `f64` |
| Shape | `is_true()`, `is_false()`, `is_some()`, `is_none()`, `is_ok()`, `is_err()` |
| Text | `starts_with(..)`, `ends_with(..)`, `contains(..)`, `cannot_contain(..)` |
| Lists, sets, maps | `has_len(n)`, `is_empty()`, `cannot_be_empty()`, `contains(item)`, `cannot_contain(item)` |
| Files | `exists()`, `cannot_exist()`, `is_a_file()`, `is_a_folder()` on anything path-like |
| Processes | `succeeds()`, `fails()`, `exits_with(code)`, then `stdout()` or `stderr()` as text, on a `std::process::Output` |
| Panics | `panics()`, `does_not_panic()` on a closure |

- A passing check returns the subject, so checks chain: `ensure(n).is_above(1).is_below(9)`.
- `is_some`, `is_ok`, `is_err`, `panics`, `stdout`, and `stderr` continue the chain with what they unwrapped: the inner value, the panic message, or the stream as text.
- A failed process check shows the exit status and both streams, so the test needs no message of its own.
- `.named("what")` heads the failure message. Use it when the value is not obvious from the line.
- Use `ensure` for assertions in Rust tests. `assert!` stays fine inside the library code of a test helper that already uses it, but new checks in tests use `ensure`.
- The `ensure` crate has no dependencies and is never part of `crust` or `crepe` themselves.
- Test the checks themselves in `ensure/tests/`. A new check needs a passing case and a failing case that matches its message.

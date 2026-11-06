/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Fluent assertions for Crust's tests (docs/TESTING.md, section 9).
//
//     use ensure::*;
//
//     ensure(exit_code).named("exit code").is(0);
//     ensure(stderr).cannot_contain("panicked");
//     ensure(output.lines()).has_len(3);
//     ensure(found).is_some().is("geometry");
//
// `ensure(value)` wraps a value in a `subject`. Each check reads as a sentence, panics with
// a message that shows what was wanted and what was found, and points at the line of the test
// that called it. A passing check returns the subject, so checks chain.
mod basics;
mod items;
mod ordering;
mod report;
mod shape;
mod subjects;
mod text;

pub use items::{holds, measured};
pub use subjects::{ensure, subject};

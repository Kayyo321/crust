/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Fluent assertions for Crust's tests (docs/TESTING.md, section 9).
//
//     use ensure::*;
//
//     ensure(exit_code).named("exit code").is(0);
//     ensure(stderr).cannot_contain("panicked");
//     ensure(command.output().unwrap()).succeeds().stdout().contains("ok");
//     ensure(found).is_some().is("geometry");
//
// `ensure(value)` wraps a value in a `subject`. Each check reads as a sentence, panics with
// a message that shows what was wanted and what was found, and points at the line of the test
// that called it. A passing check returns the subject, so checks chain.
mod basics;
mod diff;
mod items;
mod ordering;
mod paint;
mod pretty;
mod rendering;
mod shape;
mod source;
mod subjects;
mod system;
mod text;

pub use items::{holds, measured};
pub use paint::{Paint, enabled as color_enabled};
pub use rendering::{Tone, plain, report};
pub use source::without_source;
pub use subjects::{ensure, subject};

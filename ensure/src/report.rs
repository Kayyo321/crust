/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The one place that words a failure, so every check fails in the same shape:
//
//     ensure failed: exit code
//       wanted: exactly 0
//        found: 3
use std::fmt::Debug;

/// Debug text of a value, for the "found" line.
pub fn show<T: Debug + ?Sized>(value: &T) -> String {
    format!("{value:?}")
}

/// Debug text of a list, for "one of" lines.
pub fn show_all<T: Debug>(values: &[T]) -> String {
    let parts: Vec<String> = values.iter().map(show).collect();
    format!("[{}]", parts.join(", "))
}

/// Panics with the failure message. `#[track_caller]` chains from the public check, so the
/// reported location is the line in the test, not a line in this crate.
#[track_caller]
pub fn fail(name: Option<&str>, wanted: &str, found: &str) -> ! {
    let head = match name {
        Some(name) => format!("ensure failed: {name}"),
        None => "ensure failed".to_string(),
    };
    panic!("{head}\n  wanted: {wanted}\n   found: {found}");
}

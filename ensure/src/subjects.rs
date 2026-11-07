/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use crate::pretty;
use crate::rendering::report;
use std::fmt::Debug;
use std::panic::Location;

/// A value under test. Make one with [`ensure`].
pub struct subject<T> {
    pub(crate) value: T,
    pub(crate) name: Option<String>,
    /// The checks that already passed on the way here, for the failure report.
    pub(crate) trail: Vec<&'static str>,
}

/// Starts a check on `value`.
///
/// ```
/// use ensure::*;
///
/// ensure(2 + 2).is(4);
/// ensure(2 + 2).cannot_be(5);
/// ensure("crust").starts_with("cr").ends_with("st");
/// ensure(vec![1, 2, 3]).has_len(3).contains(2);
/// ensure(Some(7)).is_some().is_above(5);
/// ```
#[must_use = "ensure(..) checks nothing until you call a check on it"]
pub fn ensure<T>(value: T) -> subject<T> {
    subject { value, name: None, trail: Vec::new() }
}

impl<T> subject<T> {
    /// Names the value, so a failure says what it was: `ensure(code).named("exit code").is(0)`.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Ends the chain and gives the value back.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Records that `check` passed, and gives the subject back.
    pub(crate) fn passed(mut self, check: &'static str) -> Self {
        self.trail.push(check);
        self
    }

    /// The trail after one more passing check, for a check that continues with another value.
    pub(crate) fn trail_after(trail: Vec<&'static str>, check: &'static str) -> Vec<&'static str> {
        let mut trail = trail;
        trail.push(check);
        trail
    }

    /// A report for a failed check: titled with the name, placed at the line of the test, and
    /// saying what was wanted. The check adds what it found, then calls [`subject::raise`].
    #[track_caller]
    pub(crate) fn complaint(&self, wanted: impl Into<String>) -> report {
        let title = match &self.name {
            Some(name) => format!("ensure failed: {name}"),
            None => "ensure failed".to_string(),
        };
        report::error(title).at_location(Location::caller()).row("wanted", wanted)
    }

    /// Finishes the report with the type and the checks that passed, then panics with it.
    #[track_caller]
    pub(crate) fn raise(&self, r: report) -> ! {
        let type_name = std::any::type_name::<T>();
        // A closure's type name is an address in the source, which says nothing.
        let r = if type_name.contains("closure") {
            r
        } else {
            r.row("type", pretty::short_type(type_name))
        };
        self.raise_untyped(r)
    }

    /// Like [`subject::raise`], for a subject whose type would only confuse.
    #[track_caller]
    pub(crate) fn raise_untyped(&self, r: report) -> ! {
        let r = if self.trail.is_empty() {
            r
        } else {
            let after: String = self.trail.iter().map(|c| format!(".{c}()")).collect();
            r.row("after", after)
        };
        panic!("{}", r.render());
    }

    /// Fails with the value itself as what was found.
    #[track_caller]
    pub(crate) fn fail_with_value(&self, wanted: impl Into<String>) -> !
    where
        T: Debug,
    {
        let r = pretty::found(self.complaint(wanted), &self.value, true);
        self.raise(r)
    }
}

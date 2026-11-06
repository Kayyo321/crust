/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use crate::report;
use std::fmt::Debug;

/// A value under test. Make one with [`ensure`].
pub struct subject<T> {
    pub(crate) value: T,
    pub(crate) name: Option<String>,
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
    subject { value, name: None }
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

    #[track_caller]
    pub(crate) fn fail(&self, wanted: impl AsRef<str>, found: impl AsRef<str>) -> ! {
        report::fail(self.name.as_deref(), wanted.as_ref(), found.as_ref())
    }

    #[track_caller]
    pub(crate) fn fail_with_value(&self, wanted: impl AsRef<str>) -> !
    where
        T: Debug,
    {
        self.fail(wanted, report::show(&self.value))
    }
}

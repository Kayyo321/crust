/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Checks on the shape of a value: bool, Option, Result, and code that panics.
use crate::report::show;
use crate::subjects::subject;
use std::fmt::Debug;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl subject<bool> {
    /// The value is `true`.
    #[track_caller]
    pub fn is_true(self) -> Self {
        if !self.value {
            self.fail_with_value("true");
        }
        self
    }

    /// The value is `false`.
    #[track_caller]
    pub fn is_false(self) -> Self {
        if self.value {
            self.fail_with_value("false");
        }
        self
    }
}

impl<T: Debug> subject<Option<T>> {
    /// The option holds a value. The chain continues with that value:
    /// `ensure(found).is_some().is("geometry")`.
    #[track_caller]
    pub fn is_some(self) -> subject<T> {
        match self.value {
            Some(inner) => subject { value: inner, name: self.name },
            None => self.fail_with_value("a value (Some)"),
        }
    }

    /// The option is empty.
    #[track_caller]
    pub fn is_none(self) -> Self {
        if self.value.is_some() {
            self.fail_with_value("nothing (None)");
        }
        self
    }
}

impl<T: Debug, E: Debug> subject<Result<T, E>> {
    /// The result is `Ok`. The chain continues with the `Ok` value.
    #[track_caller]
    pub fn is_ok(self) -> subject<T> {
        match self.value {
            Ok(inner) => subject { value: inner, name: self.name },
            Err(_) => self.fail_with_value("success (Ok)"),
        }
    }

    /// The result is `Err`. The chain continues with the `Err` value.
    #[track_caller]
    pub fn is_err(self) -> subject<E> {
        match self.value {
            Err(inner) => subject { value: inner, name: self.name },
            Ok(_) => self.fail_with_value("failure (Err)"),
        }
    }
}

/// What a caught panic said, if it said text.
fn message_of(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        text.to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "(a panic with no text)".to_string()
    }
}

/// Checks on a closure: `ensure(|| parse("")).panics()`. The closure runs once, here.
impl<F, R> subject<F>
where
    F: FnOnce() -> R,
{
    /// The closure panics. The chain continues with the panic message:
    /// `ensure(|| divide(1, 0)).panics().contains("zero")`.
    #[track_caller]
    pub fn panics(self) -> subject<String> {
        match catch_unwind(AssertUnwindSafe(self.value)) {
            Err(payload) => subject { value: message_of(payload.as_ref()), name: self.name },
            Ok(_) => crate::report::fail(self.name.as_deref(), "a panic", "it returned normally"),
        }
    }

    /// The closure returns. The chain continues with what it returned.
    #[track_caller]
    pub fn does_not_panic(self) -> subject<R> {
        match catch_unwind(AssertUnwindSafe(self.value)) {
            Ok(value) => subject { value, name: self.name },
            Err(payload) => crate::report::fail(
                self.name.as_deref(),
                "no panic",
                &format!("a panic: {}", show(&message_of(payload.as_ref()))),
            ),
        }
    }
}

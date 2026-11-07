/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Checks on the shape of a value: bool, Option, Result, and code that panics.
use crate::pretty;
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
        self.passed("is_true")
    }

    /// The value is `false`.
    #[track_caller]
    pub fn is_false(self) -> Self {
        if self.value {
            self.fail_with_value("false");
        }
        self.passed("is_false")
    }
}

impl<T> subject<Option<T>> {
    /// The option holds a value. The chain continues with that value:
    /// `ensure(found).is_some().is("geometry")`.
    #[track_caller]
    pub fn is_some(self) -> subject<T> {
        match self.value {
            Some(inner) => subject {
                value: inner,
                name: self.name,
                trail: subject::<T>::trail_after(self.trail, "is_some"),
            },
            None => {
                let r = self.complaint("a value (Some)").row("found", "None");
                self.raise(r)
            }
        }
    }
}

impl<T: Debug> subject<Option<T>> {
    /// The option is empty.
    #[track_caller]
    pub fn is_none(self) -> Self {
        if self.value.is_some() {
            self.fail_with_value("nothing (None)");
        }
        self.passed("is_none")
    }
}

/// `Ok(value)` or `Err(value)` for a report, without needing the other side to be `Debug`.
struct variant<'a, V: Debug>(&'static str, &'a V);

impl<V: Debug> Debug for variant<'_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(self.0).field(self.1).finish()
    }
}

// Each side of a result needs `Debug` only when a failure has to show it, so `is_ok` asks
// for the error to be `Debug` and `is_err` asks the same of the `Ok` value.
impl<T, E: Debug> subject<Result<T, E>> {
    /// The result is `Ok`. The chain continues with the `Ok` value.
    #[track_caller]
    pub fn is_ok(self) -> subject<T> {
        if let Err(error) = &self.value {
            let r = self.complaint("success (Ok)");
            self.raise(pretty::found(r, &variant("Err", error), true));
        }
        let subject { value, name, trail } = self;
        let Ok(inner) = value else { unreachable!("an Err was reported above") };
        subject { value: inner, name, trail: subject::<T>::trail_after(trail, "is_ok") }
    }
}

impl<T: Debug, E> subject<Result<T, E>> {
    /// The result is `Err`. The chain continues with the `Err` value.
    #[track_caller]
    pub fn is_err(self) -> subject<E> {
        if let Ok(value) = &self.value {
            let r = self.complaint("failure (Err)");
            self.raise(pretty::found(r, &variant("Ok", value), true));
        }
        let subject { value, name, trail } = self;
        let Err(inner) = value else { unreachable!("an Ok was reported above") };
        subject { value: inner, name, trail: subject::<E>::trail_after(trail, "is_err") }
    }
}

/// What a caught panic said, if it said text, without terminal colors.
fn message_of(payload: &(dyn std::any::Any + Send)) -> String {
    let text = if let Some(text) = payload.downcast_ref::<&str>() {
        text.to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "(a panic with no text)".to_string()
    };
    pretty::strip_ansi(&text)
}

/// Checks on a closure: `ensure(|| parse("")).panics()`. The closure runs once, here.
impl<F, R> subject<F>
where
    F: FnOnce() -> R,
{
    /// The closure panics. The chain continues with the panic message, so a check on a
    /// check works: `ensure(|| divide(1, 0)).panics().contains("zero")`.
    #[track_caller]
    pub fn panics(self) -> subject<String> {
        let outcome = catch_unwind(AssertUnwindSafe(self.value));
        match outcome {
            Err(payload) => subject {
                value: message_of(payload.as_ref()),
                name: self.name,
                trail: subject::<F>::trail_after(self.trail, "panics"),
            },
            Ok(_) => {
                let subject { name, trail, .. } = self;
                let probe = subject { value: (), name, trail };
                let r = probe.complaint("a panic").row("found", "it returned normally");
                probe.raise_untyped(r)
            }
        }
    }

    /// The closure returns. The chain continues with what it returned.
    #[track_caller]
    pub fn does_not_panic(self) -> subject<R> {
        match catch_unwind(AssertUnwindSafe(self.value)) {
            Ok(value) => subject {
                value,
                name: self.name,
                trail: subject::<F>::trail_after(self.trail, "does_not_panic"),
            },
            Err(payload) => {
                let message = message_of(payload.as_ref());
                let subject { name, trail, .. } = self;
                let probe = subject { value: (), name, trail };
                let r = probe
                    .complaint("no panic")
                    .row("found", format!("a panic: {:?}", pretty::clip(&message, 80)))
                    .text_block("panic message", &message);
                probe.raise_untyped(r)
            }
        }
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Checks every subject supports: equality, membership, a custom test, and floats.
use crate::pretty;
use crate::subjects::subject;
use std::fmt::Debug;

fn list<U: Debug>(values: &[U]) -> String {
    let parts: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
    format!("[{}]", parts.join(", "))
}

impl<T: Debug> subject<T> {
    /// The value equals `expected`. The two may differ in type, like `String` and `&str`.
    #[track_caller]
    pub fn is<U: Debug>(self, expected: U) -> Self
    where
        T: PartialEq<U>,
    {
        if self.value != expected {
            let r = self.complaint(format!("exactly {expected:?}"));
            self.raise(pretty::difference(r, &expected, &self.value));
        }
        self.passed("is")
    }

    /// The value differs from `other`.
    #[track_caller]
    pub fn cannot_be<U: Debug>(self, other: U) -> Self
    where
        T: PartialEq<U>,
    {
        if self.value == other {
            self.fail_with_value(format!("anything but {other:?}"));
        }
        self.passed("cannot_be")
    }

    /// The value equals one of `options`.
    #[track_caller]
    pub fn is_one_of<U: Debug>(self, options: impl IntoIterator<Item = U>) -> Self
    where
        T: PartialEq<U>,
    {
        let options: Vec<U> = options.into_iter().collect();
        if !options.iter().any(|option| self.value == *option) {
            self.fail_with_value(format!("one of {}", list(&options)));
        }
        self.passed("is_one_of")
    }

    /// The value equals none of `options`.
    #[track_caller]
    pub fn cannot_be_one_of<U: Debug>(self, options: impl IntoIterator<Item = U>) -> Self
    where
        T: PartialEq<U>,
    {
        let options: Vec<U> = options.into_iter().collect();
        if options.iter().any(|option| self.value == *option) {
            self.fail_with_value(format!("none of {}", list(&options)));
        }
        self.passed("cannot_be_one_of")
    }

    /// The value passes `test`. `description` finishes the sentence "wanted a value that ...".
    #[track_caller]
    pub fn satisfies(self, description: &str, test: impl FnOnce(&T) -> bool) -> Self {
        if !test(&self.value) {
            self.fail_with_value(format!("a value that {description}"));
        }
        self.passed("satisfies")
    }
}

impl subject<f64> {
    /// The value is within `tolerance` of `expected`.
    #[track_caller]
    pub fn is_close_to(self, expected: f64, tolerance: f64) -> Self {
        // Written as a bool so that NaN, which is close to nothing, fails the check.
        let close = (self.value - expected).abs() <= tolerance;
        if !close {
            let r = self.complaint(format!("{expected:?}, give or take {tolerance:?}"));
            let r = pretty::found(r, &self.value, true)
                .row("off by", format!("{:?}", (self.value - expected).abs()));
            self.raise(r);
        }
        self.passed("is_close_to")
    }
}

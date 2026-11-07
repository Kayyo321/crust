/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Order checks. Bounds are inclusive only where the name says "at" or "between".
use crate::subjects::subject;
use std::fmt::Debug;

impl<T: Debug> subject<T> {
    /// The value is greater than `floor`.
    #[track_caller]
    pub fn is_above<U: Debug>(self, floor: U) -> Self
    where
        T: PartialOrd<U>,
    {
        if !(self.value > floor) {
            self.fail_with_value(format!("above {floor:?}"));
        }
        self.passed("is_above")
    }

    /// The value is less than `ceiling`.
    #[track_caller]
    pub fn is_below<U: Debug>(self, ceiling: U) -> Self
    where
        T: PartialOrd<U>,
    {
        if !(self.value < ceiling) {
            self.fail_with_value(format!("below {ceiling:?}"));
        }
        self.passed("is_below")
    }

    /// The value is `minimum` or more.
    #[track_caller]
    pub fn is_at_least<U: Debug>(self, minimum: U) -> Self
    where
        T: PartialOrd<U>,
    {
        if !(self.value >= minimum) {
            self.fail_with_value(format!("at least {minimum:?}"));
        }
        self.passed("is_at_least")
    }

    /// The value is `maximum` or less.
    #[track_caller]
    pub fn is_at_most<U: Debug>(self, maximum: U) -> Self
    where
        T: PartialOrd<U>,
    {
        if !(self.value <= maximum) {
            self.fail_with_value(format!("at most {maximum:?}"));
        }
        self.passed("is_at_most")
    }

    /// The value is from `low` to `high`, both included.
    #[track_caller]
    pub fn is_between<U: Debug>(self, low: U, high: U) -> Self
    where
        T: PartialOrd<U>,
    {
        if !(self.value >= low && self.value <= high) {
            self.fail_with_value(format!("from {low:?} to {high:?}"));
        }
        self.passed("is_between")
    }
}

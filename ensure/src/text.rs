/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Checks on text: anything that is `AsRef<str>` (`&str`, `String`, `Cow<str>`).
use crate::report::show;
use crate::subjects::subject;
use std::fmt::Debug;

impl<T: AsRef<str> + Debug> subject<T> {
    /// The text starts with `prefix`.
    #[track_caller]
    pub fn starts_with(self, prefix: impl AsRef<str>) -> Self {
        if !self.value.as_ref().starts_with(prefix.as_ref()) {
            self.fail_with_value(format!("text starting with {}", show(prefix.as_ref())));
        }
        self
    }

    /// The text ends with `suffix`.
    #[track_caller]
    pub fn ends_with(self, suffix: impl AsRef<str>) -> Self {
        if !self.value.as_ref().ends_with(suffix.as_ref()) {
            self.fail_with_value(format!("text ending with {}", show(suffix.as_ref())));
        }
        self
    }
}

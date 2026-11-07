/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Checks on text: anything that is `AsRef<str>` (`&str`, `String`, `Cow<str>`).
use crate::pretty;
use crate::subjects::subject;
use std::fmt::Debug;

impl<T: AsRef<str> + Debug> subject<T> {
    /// The text starts with `prefix`.
    #[track_caller]
    pub fn starts_with(self, prefix: impl AsRef<str>) -> Self {
        let prefix = prefix.as_ref();
        if !self.value.as_ref().starts_with(prefix) {
            let r = pretty::found(
                self.complaint(format!("text starting with {prefix:?}")),
                &self.value,
                true,
            );
            self.raise(pretty::case_hint(
                r,
                &format!("{:?}", self.value.as_ref()),
                &format!("{prefix:?}"),
            ));
        }
        self.passed("starts_with")
    }

    /// The text ends with `suffix`.
    #[track_caller]
    pub fn ends_with(self, suffix: impl AsRef<str>) -> Self {
        let suffix = suffix.as_ref();
        if !self.value.as_ref().ends_with(suffix) {
            let r = pretty::found(
                self.complaint(format!("text ending with {suffix:?}")),
                &self.value,
                true,
            );
            self.raise(r);
        }
        self.passed("ends_with")
    }
}

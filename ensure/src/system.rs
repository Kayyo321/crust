/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Checks on the machine: files and folders, and finished processes.
use crate::subjects::subject;
use std::fmt::Debug;
use std::path::Path;
use std::process::Output;

impl<T: AsRef<Path> + Debug> subject<T> {
    /// Something exists at the path.
    #[track_caller]
    pub fn exists(self) -> Self {
        if !self.value.as_ref().exists() {
            self.fail_with_value("a path that exists");
        }
        self.passed("exists")
    }

    /// Nothing exists at the path.
    #[track_caller]
    pub fn cannot_exist(self) -> Self {
        if self.value.as_ref().exists() {
            self.fail_with_value("a path that does not exist");
        }
        self.passed("cannot_exist")
    }

    /// The path is a regular file.
    #[track_caller]
    pub fn is_a_file(self) -> Self {
        let path = self.value.as_ref();
        if !path.is_file() {
            let found = if path.is_dir() {
                "a folder"
            } else if path.exists() {
                "something that is neither a file nor a folder"
            } else {
                "nothing there"
            };
            let r = self.complaint("a file").row("found", found).row("path", format!("{path:?}"));
            self.raise(r);
        }
        self.passed("is_a_file")
    }

    /// The path is a folder.
    #[track_caller]
    pub fn is_a_folder(self) -> Self {
        let path = self.value.as_ref();
        if !path.is_dir() {
            let found = if path.is_file() {
                "a file"
            } else if path.exists() {
                "something that is neither a file nor a folder"
            } else {
                "nothing there"
            };
            let r = self.complaint("a folder").row("found", found).row("path", format!("{path:?}"));
            self.raise(r);
        }
        self.passed("is_a_folder")
    }
}

/// Checks on a finished process: `ensure(command.output().unwrap()).succeeds()`.
impl subject<Output> {
    /// The process exited with status 0.
    #[track_caller]
    pub fn succeeds(self) -> Self {
        if !self.value.status.success() {
            let r = self.complaint("exit status 0").process(&self.value);
            self.raise(r);
        }
        self.passed("succeeds")
    }

    /// The process did not exit with status 0.
    #[track_caller]
    pub fn fails(self) -> Self {
        if self.value.status.success() {
            let r = self.complaint("a non-zero exit status").process(&self.value);
            self.raise(r);
        }
        self.passed("fails")
    }

    /// The process exited with exactly `code`.
    #[track_caller]
    pub fn exits_with(self, code: i32) -> Self {
        if self.value.status.code() != Some(code) {
            let r = self.complaint(format!("exit status {code}")).process(&self.value);
            self.raise(r);
        }
        self.passed("exits_with")
    }

    /// Continues the chain with what the process wrote to stdout.
    pub fn stdout(self) -> subject<String> {
        subject {
            value: String::from_utf8_lossy(&self.value.stdout).into_owned(),
            name: self.name,
            trail: subject::<Output>::trail_after(self.trail, "stdout"),
        }
    }

    /// Continues the chain with what the process wrote to stderr.
    pub fn stderr(self) -> subject<String> {
        subject {
            value: String::from_utf8_lossy(&self.value.stderr).into_owned(),
            name: self.name,
            trail: subject::<Output>::trail_after(self.trail, "stderr"),
        }
    }
}

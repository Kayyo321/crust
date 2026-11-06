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
        self
    }

    /// Nothing exists at the path.
    #[track_caller]
    pub fn cannot_exist(self) -> Self {
        if self.value.as_ref().exists() {
            self.fail_with_value("a path that does not exist");
        }
        self
    }

    /// The path is a regular file.
    #[track_caller]
    pub fn is_a_file(self) -> Self {
        if !self.value.as_ref().is_file() {
            self.fail_with_value("a file");
        }
        self
    }

    /// The path is a folder.
    #[track_caller]
    pub fn is_a_folder(self) -> Self {
        if !self.value.as_ref().is_dir() {
            self.fail_with_value("a folder");
        }
        self
    }
}

/// What a finished process did, worded for a failure: its status, then both streams.
fn describe(output: &Output) -> String {
    format!(
        "{}\n--- stdout\n{}--- stderr\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Checks on a finished process: `ensure(command.output().unwrap()).succeeds()`.
impl subject<Output> {
    /// The process exited with status 0.
    #[track_caller]
    pub fn succeeds(self) -> Self {
        if !self.value.status.success() {
            self.fail("exit status 0", describe(&self.value));
        }
        self
    }

    /// The process did not exit with status 0.
    #[track_caller]
    pub fn fails(self) -> Self {
        if self.value.status.success() {
            self.fail("a non-zero exit status", describe(&self.value));
        }
        self
    }

    /// The process exited with exactly `code`.
    #[track_caller]
    pub fn exits_with(self, code: i32) -> Self {
        if self.value.status.code() != Some(code) {
            self.fail(format!("exit status {code}"), describe(&self.value));
        }
        self
    }

    /// Continues the chain with what the process wrote to stdout.
    pub fn stdout(self) -> subject<String> {
        subject { value: String::from_utf8_lossy(&self.value.stdout).into_owned(), name: self.name }
    }

    /// Continues the chain with what the process wrote to stderr.
    pub fn stderr(self) -> subject<String> {
        subject { value: String::from_utf8_lossy(&self.value.stderr).into_owned(), name: self.name }
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Path and process checks, on real files and real processes.
use ensure::*;
use std::process::Command;

fn here() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn path_checks() {
    ensure(here()).exists().is_a_folder();
    ensure(here().join("Cargo.toml")).exists().is_a_file();
    ensure(here().join("no_such_file")).cannot_exist();
}

#[test]
fn path_failures() {
    ensure(|| ensure(here().join("nope")).exists()).panics().contains("wanted: a path that exists");
    ensure(|| ensure(here()).cannot_exist()).panics().contains("a path that does not exist");
    ensure(|| ensure(here()).is_a_file()).panics().contains("wanted: a file");
    ensure(|| ensure(here().join("Cargo.toml")).is_a_folder()).panics().contains("a folder");
}

fn shell(script: &str) -> std::process::Output {
    Command::new("sh").arg("-c").arg(script).output().unwrap()
}

#[test]
fn process_checks() {
    ensure(shell("echo hi")).succeeds().stdout().is("hi\n");
    ensure(shell("echo oops >&2; exit 3")).fails().exits_with(3).stderr().contains("oops");
}

#[test]
fn process_failures_show_the_streams() {
    ensure(|| ensure(shell("echo oops >&2; exit 3")).succeeds())
        .panics()
        .contains("wanted: exit status 0")
        .contains("exit status: 3")
        .contains("--- stderr\noops");
    ensure(|| ensure(shell("true")).fails()).panics().contains("a non-zero exit status");
    ensure(|| ensure(shell("exit 2")).exits_with(3)).panics().contains("wanted: exit status 3");
}

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

/// The report that `check` panics with, with no source lines in it.
fn failure<R>(check: impl FnOnce() -> R) -> subject<String> {
    without_source(|| ensure(check).panics())
}

#[test]
fn path_failures() {
    failure(|| ensure(here().join("nope")).exists()).contains("wanted: a path that exists");
    failure(|| ensure(here()).cannot_exist()).contains("wanted: a path that does not exist");
    failure(|| ensure(here()).is_a_file()).contains("wanted: a file").contains("found: a folder");
    failure(|| ensure(here().join("Cargo.toml")).is_a_folder())
        .contains("wanted: a folder")
        .contains("found: a file");
    failure(|| ensure(here().join("nope")).is_a_file()).contains("found: nothing there");
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
fn process_failures_show_the_status_and_both_streams() {
    let report =
        failure(|| ensure(shell("echo out; echo oops >&2; exit 3")).succeeds()).into_value();
    let lines: Vec<String> = report.lines().map(|l| l.trim().to_string()).collect();
    ensure(lines.clone()).contains("= wanted: exit status 0").contains("=  found: exit status 3");
    // Each stream is a block: its title, then its lines behind the gutter.
    let at = |title: &str| lines.iter().position(|l| l == title).unwrap();
    ensure(lines[at("= stdout:") + 1].as_str()).is("|   out");
    ensure(lines[at("= stderr:") + 1].as_str()).is("|   oops");
}

#[test]
fn an_empty_stream_is_said_to_be_empty() {
    failure(|| ensure(shell("exit 2")).succeeds())
        .contains("stdout: (empty)")
        .contains("stderr: (empty)");
}

#[test]
fn process_status_checks() {
    failure(|| ensure(shell("true")).fails()).contains("wanted: a non-zero exit status");
    failure(|| ensure(shell("exit 2")).exits_with(3)).contains("wanted: exit status 3");
}

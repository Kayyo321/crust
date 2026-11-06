/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The release version is stated once (the workspace Cargo.toml) and everything that names it
// agrees: both CLIs, and the status line in README.md.
use ensure::*;
use std::path::PathBuf;
use std::process::Command;

fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[test]
fn version_is_major_minor_patch() {
    let parts: Vec<&str> = version().split('.').collect();
    ensure(parts.len()).named("parts of the version").is(3);
    for part in parts {
        ensure(part.parse::<u32>().is_ok()).named(format!("{part} is a number")).is_true();
    }
}

#[test]
fn crust_prints_the_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_crust")).arg("--version").output().unwrap();
    ensure(out).succeeds().stdout().is(format!("crust {}\n", version()));
}

#[test]
fn readme_status_names_the_version() {
    let readme =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.md"));
    ensure(readme.unwrap()).contains(format!("**Status: {}, pre-alpha.**", version()));
}

#[test]
fn changelog_has_an_entry_for_the_version() {
    let log =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("CHANGELOG.md"));
    ensure(log.unwrap()).contains(format!("## {}\n", version()));
}

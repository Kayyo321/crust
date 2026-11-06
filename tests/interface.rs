/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Interface files (docs/TESTING.md, section 3): the golden `.crmi` test, and
// the rule that a body-only change keeps the interface hash (D-030).
use ensure::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crust() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_crust"))
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/interface")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("crust-iface-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn interface_of(src: &Path, out: &Path) -> Vec<u8> {
    let built =
        Command::new(crust()).arg("--emit=interface").arg(src).arg("-o").arg(out).output().unwrap();
    ensure(built).succeeds();
    std::fs::read(out).unwrap()
}

fn printed(crmi: &Path) -> String {
    let out = Command::new(crust()).arg("--print-interface").arg(crmi).output().unwrap();
    ensure(out).succeeds().stdout().into_value()
}

/// The printed golden file, without its leading file header.
fn golden() -> String {
    let text = std::fs::read_to_string(dir().join("geometry.interface")).unwrap();
    let body = text.split_once("*/\n").map(|(_, b)| b).unwrap_or(&text);
    body.to_string()
}

#[test]
fn golden_interface_matches() {
    let d = scratch("golden");
    let crmi = d.join("geometry.crmi");
    interface_of(&dir().join("geometry.crm"), &crmi);
    let got = printed(&crmi);
    if std::env::var("CRUST_BLESS").is_ok() {
        let header = "/**\n* Ko, (Baczynski-Bruce) 2026; Apache 2.0\n*/\n";
        std::fs::write(dir().join("geometry.interface"), format!("{header}{got}")).unwrap();
        return;
    }
    ensure(got)
        .named("printed interface (if intended, rerun with CRUST_BLESS=1 and review the diff)")
        .is(golden());
}

#[test]
fn body_change_keeps_interface_hash() {
    let d = scratch("body");
    let src = std::fs::read_to_string(dir().join("geometry.crm")).unwrap();
    let a = d.join("geometry.crm");
    std::fs::write(&a, &src).unwrap();
    let before = interface_of(&a, &d.join("a.crmi"));
    std::fs::write(&a, src.replace("return v * v;", "return v * v + 0.0;")).unwrap();
    let after = interface_of(&a, &d.join("b.crmi"));
    ensure(after).named("interface after a body-only change").is(before.clone());
    std::fs::write(
        &a,
        src.replace("export double distance(", "export double distance2(")
            .replace("return distance(a, b);", "return distance2(a, b);"),
    )
    .unwrap();
    let changed = interface_of(&a, &d.join("c.crmi"));
    ensure(changed[8..16].to_vec())
        .named("hash after an export change")
        .cannot_be(before[8..16].to_vec());
}

#[test]
fn importers_rebuild_only_on_interface_change() {
    // With crepe: a body edit recompiles only the edited module.
    let d = scratch("crepe");
    let crepe = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/crepe");
    if !crepe.is_file() {
        eprintln!("skipping: build crepe first (cargo build -p crepe)");
        return;
    }
    std::fs::create_dir_all(d.join("src")).unwrap();
    let header = "/**\n* Ko, (Baczynski-Bruce) 2026; Apache 2.0\n*/\n";
    std::fs::write(d.join("crepe.toml"), "# Ko, (Baczynski-Bruce) 2026; Apache 2.0\n[package]\nname = \"demo\"\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(
        d.join("src/util.crm"),
        format!("{header}module util;\nexport int twice(int x) {{\n    return 2 * x;\n}}\n"),
    )
    .unwrap();
    std::fs::write(
        d.join("src/main.crm"),
        format!("{header}module demo;\nimport {{ twice }} from \"util\";\nint main(void) {{\n    return twice(2) == 4 ? 0 : 1;\n}}\n"),
    )
    .unwrap();
    let build = || {
        let out = Command::new(&crepe)
            .arg("build")
            .current_dir(&d)
            .env("CRUST", crust())
            .output()
            .unwrap();
        ensure(out).succeeds().stderr().into_value()
    };
    build();
    std::fs::write(
        d.join("src/util.crm"),
        format!("{header}module util;\nexport int twice(int x) {{\n    return x + x;\n}}\n"),
    )
    .unwrap();
    let log = build();
    ensure(log).named("build log").contains("Compiling util").cannot_contain("Compiling demo");
}

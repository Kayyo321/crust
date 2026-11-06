/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The ABI matrix (docs/ABI.md, section 5), for x86-64 System V: layouts
// match clang, and calls work in both directions between Crust and C code
// that clang compiled.
use ensure::*;
use std::path::PathBuf;
use std::process::Command;

fn crust() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_crust"))
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/abi")
}

fn clang() -> Option<String> {
    ["clang-18", "clang"]
        .iter()
        .find(|c| Command::new(c).arg("--version").output().is_ok())
        .map(|c| c.to_string())
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("crust-abi-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(cmd: &mut Command) -> String {
    ensure(cmd.output().unwrap()).named(format!("{cmd:?}")).succeeds().stdout().into_value()
}

#[test]
fn layouts_match_clang() {
    let Some(cc) = clang() else { return };
    let d = scratch("layout");
    run(Command::new(crust()).arg(dir().join("layout.c")).arg("-o").arg(d.join("crust_layout")));
    run(Command::new(&cc)
        .arg("-w")
        .arg("-std=c2x")
        .arg(dir().join("layout.c"))
        .arg("-o")
        .arg(d.join("clang_layout")));
    let a = run(&mut Command::new(d.join("crust_layout")));
    let b = run(&mut Command::new(d.join("clang_layout")));
    ensure(a).named("crust layout").is(b);
}

#[test]
fn calls_work_in_both_directions() {
    let Some(cc) = clang() else { return };
    let d = scratch("calls");
    run(Command::new(&cc)
        .arg("-c")
        .arg("-w")
        .arg(dir().join("c_side.c"))
        .arg("-o")
        .arg(d.join("c_side.o")));
    run(Command::new(crust())
        .arg(dir().join("crust_side.crm"))
        .arg(d.join("c_side.o"))
        .arg("-o")
        .arg(d.join("calls")));
    let out = run(&mut Command::new(d.join("calls")));
    ensure(out).is(std::fs::read_to_string(dir().join("calls.stdout")).unwrap());
}

#[test]
fn c_program_links_crust_libraries() {
    let Some(cc) = clang() else { return };
    let d = scratch("libs");
    run(Command::new(crust())
        .arg("--emit=staticlib")
        .arg(dir().join("libsum.crm"))
        .arg("-o")
        .arg(d.join("libabi.a")));
    run(Command::new(crust())
        .arg("--emit=header")
        .arg(dir().join("libsum.crm"))
        .arg("-o")
        .arg(d.join("abi_test.h")));
    let header = std::fs::read_to_string(d.join("abi_test.h")).unwrap();
    ensure(header)
        .named("generated header")
        .contains("int crust_add(int, int);")
        .contains("struct s_int2 {")
        .cannot_contain("ref ");
    let main = d.join("main.c");
    std::fs::write(
        &main,
        "/**\n* Ko, (Baczynski-Bruce) 2026; Apache 2.0\n*/\n#include <stdio.h>\n#include \"abi_test.h\"\nint main(void) { printf(\"%d\\n\", crust_add(40, 2)); return 0; }\n",
    )
    .unwrap();
    run(Command::new(&cc)
        .arg("-w")
        .arg(&main)
        .arg(format!("-I{}", d.display()))
        .arg(d.join("libabi.a"))
        .arg("-o")
        .arg(d.join("static_user")));
    ensure(run(&mut Command::new(d.join("static_user")))).is("42\n");
    run(Command::new(crust())
        .arg("-shared")
        .arg(dir().join("libsum.crm"))
        .arg("-o")
        .arg(d.join("libabi.so")));
    run(Command::new(&cc)
        .arg("-w")
        .arg(&main)
        .arg(format!("-I{}", d.display()))
        .arg(format!("-L{}", d.display()))
        .arg("-labi")
        .arg(format!("-Wl,-rpath,{}", d.display()))
        .arg("-o")
        .arg(d.join("shared_user")));
    ensure(run(&mut Command::new(d.join("shared_user")))).is("42\n");
}

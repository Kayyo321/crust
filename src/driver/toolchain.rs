/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Finding and running the LLVM tools that turn textual IR into objects, and
// the system linker (docs/ARCHITECTURE.md, section 8).
use std::path::{Path, PathBuf};
use std::process::Command;

fn llvm_bindir() -> Option<PathBuf> {
    let cfg = std::env::var("CRUST_LLVM_CONFIG").ok()?;
    let out = Command::new(cfg).arg("--bindir").output().ok()?;
    Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// The clang used as the LLVM backend driver and linker driver.
pub fn clang() -> PathBuf {
    if let Some(d) = llvm_bindir() {
        return d.join("clang");
    }
    for n in ["clang-18", "clang"] {
        if on_path(n) {
            return PathBuf::from(n);
        }
    }
    PathBuf::from("clang")
}

pub fn archiver() -> PathBuf {
    if let Some(d) = llvm_bindir() {
        return d.join("llvm-ar");
    }
    for n in ["llvm-ar-18", "llvm-ar", "ar"] {
        if on_path(n) {
            return PathBuf::from(n);
        }
    }
    PathBuf::from("ar")
}

pub fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run `{}`: {e}", cmd.get_program().to_string_lossy()))?;
    if !out.status.success() {
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        return Err(format!(
            "`{} {}` failed:\n{}{}",
            cmd.get_program().to_string_lossy(),
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// Compiles an LLVM IR file to an object or assembly file.
pub fn compile_ir(
    ir: &Path,
    out: &Path,
    opt: &str,
    pic: bool,
    debug: bool,
    asm: bool,
) -> Result<(), String> {
    let mut c = Command::new(clang());
    c.arg(if asm { "-S" } else { "-c" }).arg("-x").arg("ir").arg(ir).arg("-o").arg(out).arg(opt);
    c.arg("-Wno-override-module").arg("--target=x86_64-pc-linux-gnu");
    if pic {
        c.arg("-fPIC");
    }
    if debug {
        c.arg("-g");
    }
    run(&mut c)
}

pub fn link(
    objs: &[PathBuf],
    out: &Path,
    lib_dirs: &[PathBuf],
    libs: &[String],
    shared: bool,
) -> Result<(), String> {
    let mut c = Command::new(clang());
    c.arg("-o").arg(out);
    if shared {
        c.arg("-shared");
    }
    c.args(objs);
    for d in lib_dirs {
        c.arg(format!("-L{}", d.display()));
    }
    for l in libs {
        c.arg(format!("-l{l}"));
    }
    run(&mut c)
}

pub fn archive(objs: &[PathBuf], out: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(out);
    let mut c = Command::new(archiver());
    c.arg("rcs").arg(out).args(objs);
    run(&mut c)
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Keeps every example in examples/ building and printing what it says
// (examples/README.md describes the layout).
//
// - A folder with build.sh runs it with CRUST, CREPE, and CC set; its
//   stdout must equal expect.stdout.
// - A folder whose files carry `//~ ERROR` markers holds failing examples:
//   each file must fail with exactly the marked codes.
// - A folder with .crm files or an expect.stdout is one program: all its
//   .c, .cr, and .crm files compile together.
// - Otherwise each file is its own program, with output in <name>.stdout.
use ensure::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crust() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_crust"))
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("crust-examples-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .collect();
    v.sort();
    v
}

fn is_source(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()), Some("c" | "cr" | "crm"))
}

fn markers(f: &Path) -> BTreeSet<(u32, String)> {
    let text = std::fs::read_to_string(f).unwrap_or_default();
    let mut out = BTreeSet::new();
    for (i, line) in text.lines().enumerate() {
        if let Some(pos) = line.find("//~ ERROR ") {
            for code in line[pos + 10..].split_whitespace() {
                out.insert((i as u32 + 1, code.to_string()));
            }
        }
    }
    out
}

fn json_field<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let k = format!("\"{key}\":");
    let rest = &s[s.find(&k)? + k.len()..];
    match rest.strip_prefix('"') {
        Some(r) => Some(&r[..r.find('"')?]),
        None => Some(&rest[..rest.find([',', '}']).unwrap_or(rest.len())]),
    }
}

fn check_fails(f: &Path, out: &Path) -> Result<(), String> {
    let o = Command::new(crust())
        .arg(f)
        .arg("--error-format=json")
        .arg("-o")
        .arg(out.join("x"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr);
    let mut got = BTreeSet::new();
    for line in stderr.lines().filter(|l| l.starts_with('{')) {
        if json_field(line, "severity") != Some("error") {
            continue;
        }
        let span = &line[line.find("\"span\":").unwrap_or(0)..];
        let ln = json_field(span, "line").and_then(|l| l.parse().ok()).unwrap_or(0);
        got.insert((ln, json_field(line, "code").unwrap_or("null").to_string()));
    }
    let want = markers(f);
    if got != want {
        return Err(format!("{}: expected {want:?}, got {got:?}\n{stderr}", f.display()));
    }
    Ok(())
}

fn run(cmd: &mut Command) -> Result<String, String> {
    let o = cmd.output().unwrap();
    if !o.status.success() {
        return Err(format!(
            "{cmd:?} failed:\n{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

fn expect(path: &Path, got: &str) -> Result<(), String> {
    match std::fs::read_to_string(path) {
        Ok(want) if want == got => Ok(()),
        Ok(want) => {
            Err(format!("{}: output differs\n--- expected\n{want}--- got\n{got}", path.display()))
        }
        Err(_) => Err(format!("{} is missing", path.display())),
    }
}

fn check_example(dir: &Path) -> Result<(), String> {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let out = scratch(&name);
    let fs = files(dir);
    if dir.join("build.sh").is_file() {
        let crepe = root().join("target/debug/crepe");
        let cc = ["clang-18", "clang", "cc"]
            .iter()
            .find(|c| Command::new(c).arg("--version").output().is_ok())
            .unwrap();
        let got = run(Command::new("sh")
            .arg(dir.join("build.sh"))
            .arg(&out)
            .env("CRUST", crust())
            .env("CREPE", crepe)
            .env("CC", cc))?;
        return expect(&dir.join("expect.stdout"), &got);
    }
    if fs.iter().any(|f| !markers(f).is_empty()) {
        for f in fs.iter().filter(|f| is_source(f)) {
            check_fails(f, &out)?;
        }
        return Ok(());
    }
    let together = dir.join("expect.stdout").is_file()
        || fs.iter().any(|f| f.extension().is_some_and(|e| e == "crm"));
    if together {
        let exe = out.join("prog");
        let srcs: Vec<&PathBuf> = fs.iter().filter(|f| is_source(f)).collect();
        run(Command::new(crust())
            .args(srcs)
            .arg(format!("-I{}", dir.display()))
            .arg("-lm")
            .arg("-o")
            .arg(&exe))?;
        return expect(&dir.join("expect.stdout"), &run(&mut Command::new(&exe))?);
    }
    for f in fs.iter().filter(|f| is_source(f)) {
        let exe = out.join(f.file_stem().unwrap());
        run(Command::new(crust()).arg(f).arg("-lm").arg("-o").arg(&exe))?;
        expect(&f.with_extension("stdout"), &run(&mut Command::new(&exe))?)?;
    }
    Ok(())
}

#[test]
fn every_example_builds_and_runs() {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root().join("examples"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    ensure(dirs.len()).named("example folders").is_at_least(13);
    let failures: Vec<String> = dirs.iter().filter_map(|d| check_example(d).err()).collect();
    if !failures.is_empty() {
        panic!("{} examples failed:\n\n{}", failures.len(), failures.join("\n\n"));
    }
}

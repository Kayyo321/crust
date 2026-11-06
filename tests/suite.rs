/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The file-based test suite (docs/TESTING.md, section 2).
//
// A case is a file, or a directory of files compiled together.
// - A case with `//~ ERROR CRnnnn` markers must fail to compile with exactly
//   those codes on exactly those lines.
// - Any other case must compile, run, exit 0, and print the contents of its
//   `.stdout` file (`expect.stdout` for a directory), if there is one.
// - `// args: ...` on a line passes extra flags to crust.
// Run-pass programs run under valgrind's leak checker when it is installed
// (set CRUST_TEST_VALGRIND=0 to skip), so a leak fails the test (G-LEAK).
use ensure::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crust() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_crust"))
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("crust-suite-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn is_source(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()), Some("c" | "cr" | "crm"))
}

fn sources(case: &Path) -> Vec<PathBuf> {
    if case.is_file() {
        return vec![case.to_path_buf()];
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(case)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| is_source(p))
        .collect();
    v.sort();
    v
}

fn all_files(case: &Path) -> Vec<PathBuf> {
    if case.is_file() {
        return vec![case.to_path_buf()];
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(case).unwrap().map(|e| e.unwrap().path()).collect();
    v.sort();
    v
}

/// Expected (file name, line, code) triples from `//~ ERROR` markers.
fn markers(case: &Path) -> BTreeSet<(String, u32, String)> {
    let mut out = BTreeSet::new();
    for f in all_files(case) {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for (i, line) in text.lines().enumerate() {
            if let Some(pos) = line.find("//~ ERROR ") {
                for code in line[pos + 10..].split_whitespace() {
                    let name = f.file_name().unwrap().to_string_lossy().into_owned();
                    out.insert((name, i as u32 + 1, code.to_string()));
                }
            }
        }
    }
    out
}

fn extra_args(case: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for f in all_files(case) {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for line in text.lines() {
            if let Some(a) = line.trim().strip_prefix("// args:") {
                v.extend(a.split_whitespace().map(String::from));
            }
        }
    }
    v
}

fn json_field<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let k = format!("\"{key}\":");
    let i = s.find(&k)? + k.len();
    let rest = &s[i..];
    if let Some(r) = rest.strip_prefix('"') {
        let end = r.find('"')?;
        Some(&r[..end])
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

/// Runs crust and returns the (file, line, code) of every error.
fn errors_of(case: &Path, out_dir: &Path) -> (BTreeSet<(String, u32, String)>, String) {
    let mut c = Command::new(crust());
    // Several inputs share one output only when linked; diagnostics stop the
    // build before linking anyway.
    c.args(sources(case)).arg("--error-format=json").arg("-o").arg(out_dir.join("prog"));
    if case.is_dir() {
        c.arg(format!("-I{}", case.display()));
    }
    c.args(extra_args(case));
    let out = c.output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let mut got = BTreeSet::new();
    for line in stderr.lines() {
        if !line.starts_with('{') || json_field(line, "severity") != Some("error") {
            continue;
        }
        let code = json_field(line, "code").unwrap_or("null").to_string();
        let span = &line[line.find("\"span\":").unwrap_or(0)..];
        let file = json_field(span, "file")
            .map(|f| Path::new(f).file_name().unwrap().to_string_lossy().into_owned())
            .unwrap_or_default();
        let ln: u32 = json_field(span, "line").and_then(|l| l.parse().ok()).unwrap_or(0);
        got.insert((file, ln, code));
    }
    (got, stderr)
}

fn valgrind() -> Option<PathBuf> {
    if std::env::var("CRUST_TEST_VALGRIND").as_deref() == Ok("0") {
        return None;
    }
    let p = PathBuf::from("/usr/bin/valgrind");
    p.is_file().then_some(p)
}

fn run_case(case: &Path, name: &str) -> Result<(), String> {
    let dir = scratch(name);
    let expected = markers(case);
    if !expected.is_empty() {
        let (got, stderr) = errors_of(case, &dir);
        if got != expected {
            let missing: Vec<_> = expected.difference(&got).collect();
            let extra: Vec<_> = got.difference(&expected).collect();
            return Err(format!(
                "expected errors not reported: {missing:?}\nunexpected errors: {extra:?}\n{stderr}"
            ));
        }
        return Ok(());
    }
    let exe = dir.join("prog");
    let mut c = Command::new(crust());
    c.args(sources(case)).arg("-o").arg(&exe);
    if case.is_dir() {
        c.arg(format!("-I{}", case.display()));
    }
    c.args(extra_args(case));
    let out = c.output().unwrap();
    if !out.status.success() {
        return Err(format!("compile failed:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    let run = match valgrind() {
        Some(vg) => Command::new(vg)
            .args([
                "-q",
                "--leak-check=full",
                "--errors-for-leak-kinds=definite,indirect",
                "--error-exitcode=99",
            ])
            .arg(&exe)
            .output()
            .unwrap(),
        None => Command::new(&exe).output().unwrap(),
    };
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    if !run.status.success() {
        return Err(format!(
            "exit status {:?}\nstdout:\n{stdout}\nstderr:\n{}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    let expect_file =
        if case.is_dir() { case.join("expect.stdout") } else { case.with_extension("stdout") };
    if let Ok(want) = std::fs::read_to_string(&expect_file) {
        if want != stdout {
            return Err(format!("stdout differs\n--- expected\n{want}--- got\n{stdout}"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

fn run_dir(sub: &str) {
    let d = root().join(sub);
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&d)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir() || is_source(p))
        .collect();
    cases.sort();
    ensure(cases.len()).named(format!("test cases in tests/{sub}")).is_above(0);
    let mut failures = Vec::new();
    for (i, c) in cases.iter().enumerate() {
        let name = format!("{}-{i}", sub.replace('/', "-"));
        if let Err(e) = run_case(c, &name) {
            failures.push(format!("FAILED {}\n{e}", c.strip_prefix(root()).unwrap().display()));
        }
    }
    if !failures.is_empty() {
        panic!(
            "{} of {} cases failed in tests/{sub}:\n\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n\n")
        );
    }
}

#[test]
fn compile_fail() {
    run_dir("compile_fail");
}

#[test]
fn run_pass() {
    run_dir("run_pass");
}

#[test]
fn modules() {
    run_dir("modules");
}

#[test]
fn pairs() {
    run_dir("pairs");
}

#[test]
fn soundness_must_reject() {
    run_dir("soundness/must_reject");
}

#[test]
fn soundness_must_accept() {
    run_dir("soundness/must_accept");
}

#[test]
fn every_soundness_case_names_its_guarantee() {
    for sub in ["soundness/must_reject", "soundness/must_accept"] {
        for c in std::fs::read_dir(root().join(sub)).unwrap() {
            let p = c.unwrap().path();
            let text: String =
                all_files(&p).iter().filter_map(|f| std::fs::read_to_string(f).ok()).collect();
            let names_a_guarantee = ["G-UAF", "G-DF", "G-LEAK", "G-OOB", "G-UNINIT", "G-ALIAS"]
                .iter()
                .any(|g| text.contains(&format!("guarantee: {g}")));
            ensure(names_a_guarantee)
                .named(format!("{} has a `guarantee: G-...` tag", p.display()))
                .is_true();
        }
    }
}

#[test]
fn every_test_file_has_the_header() {
    let header = "/**\n* Ko, (Baczynski-Bruce) 2026; Apache 2.0\n*/\n";
    let mut stack = vec![root()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("c" | "h" | "cr" | "hr" | "crm" | "rs")
            ) {
                let t = std::fs::read_to_string(&p).unwrap();
                ensure(t.starts_with(header))
                    .named(format!("{} starts with the file header", p.display()))
                    .is_true();
            }
        }
    }
}

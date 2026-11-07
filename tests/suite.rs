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
use std::process::{Command, Output};

fn crust() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_crust"))
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// The repository root, with a trailing slash, as it appears in absolute paths.
fn repo_prefix() -> String {
    format!("{}/", env!("CARGO_MANIFEST_DIR"))
}

/// `path` relative to the repository, as text.
fn relative(path: &Path) -> String {
    path.display().to_string().replacen(&repo_prefix(), "", 1)
}

/// `text` with the repository root cut out of every path in it, so reports show `tests/...`.
fn relative_text(text: &str) -> String {
    text.replace(&repo_prefix(), "")
}

/// `output` with the repository root cut out of its streams.
fn relative_output(output: &Output) -> Output {
    Output {
        status: output.status,
        stdout: relative_text(&String::from_utf8_lossy(&output.stdout)).into_bytes(),
        stderr: relative_text(&String::from_utf8_lossy(&output.stderr)).into_bytes(),
    }
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

/// The value of `"key":` in one line of crust's JSON diagnostics, as written (escapes kept).
fn json_field<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let k = format!("\"{key}\":");
    let i = s.find(&k)? + k.len();
    let rest = &s[i..];
    if let Some(r) = rest.strip_prefix('"') {
        let mut escaped = false;
        for (at, c) in r.char_indices() {
            match c {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => return Some(&r[..at]),
                _ => escaped = false,
            }
        }
        None
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

/// Undoes the JSON escapes of a string that `json_field` returned.
fn json_unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// One error crust reported, as the suite sees it.
struct found_error {
    file: String,
    line: u32,
    column: u32,
    length: usize,
    code: String,
    message: String,
}

/// Runs crust and returns the (file, line, code) of every error, the errors in full, and
/// everything it wrote to stderr.
fn errors_of(
    case: &Path,
    out_dir: &Path,
) -> (BTreeSet<(String, u32, String)>, Vec<found_error>, String) {
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
    let mut errors = Vec::new();
    for line in stderr.lines() {
        if !line.starts_with('{') || json_field(line, "severity") != Some("error") {
            continue;
        }
        let code = json_field(line, "code").unwrap_or("null").to_string();
        let span = &line[line.find("\"span\":").unwrap_or(0)..];
        let file = json_field(span, "file")
            .map(|f| Path::new(f).file_name().unwrap().to_string_lossy().into_owned())
            .unwrap_or_default();
        let number = |key: &str| json_field(span, key).and_then(|l| l.parse().ok()).unwrap_or(0);
        let ln: u32 = number("line");
        got.insert((file.clone(), ln, code.clone()));
        errors.push(found_error {
            file,
            line: ln,
            column: number("column"),
            length: number("length") as usize,
            code,
            message: json_unescape(json_field(line, "message").unwrap_or("")),
        });
    }
    (got, errors, stderr)
}

/// What crust prints for the case, as a person would see it.
fn human_output(case: &Path, out_dir: &Path) -> String {
    let mut c = Command::new(crust());
    c.args(sources(case)).arg("-o").arg(out_dir.join("prog"));
    if case.is_dir() {
        c.arg(format!("-I{}", case.display()));
    }
    c.args(extra_args(case));
    String::from_utf8_lossy(&c.output().unwrap().stderr).into_owned()
}

fn valgrind() -> Option<PathBuf> {
    if std::env::var("CRUST_TEST_VALGRIND").as_deref() == Ok("0") {
        return None;
    }
    let p = PathBuf::from("/usr/bin/valgrind");
    p.is_file().then_some(p)
}

/// The path of the file called `name` in the case.
fn file_of(case: &Path, name: &str) -> PathBuf {
    all_files(case)
        .into_iter()
        .find(|f| f.file_name().is_some_and(|n| n == name))
        .unwrap_or_else(|| case.join(name))
}

/// The reports for a compile-fail case whose errors differ from its `//~ ERROR` markers: one
/// for each marker crust did not satisfy, one for each error nobody expected.
fn mismatch_reports(
    case: &Path,
    expected: &BTreeSet<(String, u32, String)>,
    got: &BTreeSet<(String, u32, String)>,
    errors: &[found_error],
) -> Vec<report> {
    let mut reports = Vec::new();
    for (file, line, code) in expected.difference(got) {
        let path = file_of(case, file);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let source_line = text.lines().nth(*line as usize - 1).unwrap_or("");
        let at = source_line.find("//~ ERROR").unwrap_or(0);
        let column = source_line[..at].chars().count() as u32 + 1;
        let width = source_line[at..].chars().count();
        let here: Vec<&str> = errors
            .iter()
            .filter(|e| e.file == *file && e.line == *line)
            .map(|e| e.code.as_str())
            .collect();
        let elsewhere: Vec<u32> = errors
            .iter()
            .filter(|e| e.file == *file && e.line != *line && e.code == *code)
            .map(|e| e.line)
            .collect();
        let mut r = report::error(format!("crust did not report {code}"))
            .at(relative(&path), *line, column)
            .underline(width)
            .label(format!("this line is marked as an error, and crust did not report {code} here"))
            .row("wanted", format!("{code} on line {line}"))
            .row(
                "found",
                if here.is_empty() { "no error on this line".to_string() } else { here.join(", ") },
            );
        if !elsewhere.is_empty() {
            let lines: Vec<String> = elsewhere.iter().map(u32::to_string).collect();
            r = r.help(format!(
                "{code} was reported on line {} instead; move the marker there",
                lines.join(", ")
            ));
        }
        reports.push(r);
    }
    for (file, line, code) in got.difference(expected) {
        let path = file_of(case, file);
        let error = errors.iter().find(|e| e.file == *file && e.line == *line && e.code == *code);
        let (column, length, message) =
            error.map(|e| (e.column, e.length, e.message.as_str())).unwrap_or((0, 1, ""));
        reports.push(
            report::error(format!("crust reported an unexpected {code}"))
                .at(relative(&path), *line, column)
                .underline(length.max(1))
                .label(message)
                .row("wanted", "no error on this line (it has no `//~ ERROR` marker)")
                .row("found", format!("{code}: {message}"))
                .help(format!("if the error is intended, end the line with `//~ ERROR {code}`")),
        );
    }
    reports
}

fn run_case(case: &Path, name: &str) -> Result<(), Vec<report>> {
    let dir = scratch(name);
    let shown = case.strip_prefix(root()).unwrap_or(case).display().to_string();
    let expected = markers(case);
    if !expected.is_empty() {
        let (got, errors, _) = errors_of(case, &dir);
        if got != expected {
            let mut reports = mismatch_reports(case, &expected, &got, &errors);
            reports.push(
                report::error(format!("what crust printed for tests/{shown}"))
                    .text_block("crust", &relative_text(human_output(case, &dir).trim_end())),
            );
            return Err(reports);
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
        return Err(vec![
            report::error(format!("tests/{shown} does not compile"))
                .row("wanted", "a program that compiles (it has no `//~ ERROR` markers)")
                .process(&relative_output(&out))
                .help("if the program should be rejected, mark the failing line with `//~ ERROR CRnnnn`"),
        ]);
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
        let mut r = report::error(format!("tests/{shown} fails when run"))
            .row("wanted", "exit status 0")
            .process(&relative_output(&run));
        if run.status.code() == Some(99) {
            r = r.help("status 99 is valgrind's: the program leaked memory or touched memory it should not (G-LEAK)");
        }
        return Err(vec![r]);
    }
    let expect_file =
        if case.is_dir() { case.join("expect.stdout") } else { case.with_extension("stdout") };
    if let Ok(want) = std::fs::read_to_string(&expect_file) {
        if want != stdout {
            return Err(vec![
                report::error(format!("tests/{shown} prints the wrong output"))
                    .at(relative(&expect_file), 1, 0)
                    .row(
                        "wanted",
                        format!(
                            "the contents of {}",
                            expect_file.file_name().unwrap().to_string_lossy()
                        ),
                    )
                    .row("found", format!("{} bytes on stdout", stdout.len()))
                    .text_diff(&want, &stdout)
                    .help("if the new output is intended, update the .stdout file"),
            ]);
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
    let mut failures: Vec<(String, Vec<report>)> = Vec::new();
    for (i, c) in cases.iter().enumerate() {
        let name = format!("{}-{i}", sub.replace('/', "-"));
        if let Err(reports) = run_case(c, &name) {
            failures.push((c.strip_prefix(root()).unwrap().display().to_string(), reports));
        }
    }
    if !failures.is_empty() {
        let mut text = String::new();
        for (_, reports) in &failures {
            for r in reports {
                text.push_str(&r.render());
                text.push_str("\n\n");
            }
        }
        let names: Vec<String> = failures.iter().map(|(n, _)| format!("tests/{n}")).collect();
        let summary = report::error(format!(
            "{} of {} cases failed in tests/{sub}",
            failures.len(),
            cases.len()
        ))
        .text_block("failed cases", &names.join("\n"));
        panic!("\n{text}{}", summary.render());
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

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
use std::process::{Command, Output};

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

/// The repository root, with a trailing slash, as it appears in absolute paths.
fn repo_prefix() -> String {
    format!("{}/", env!("CARGO_MANIFEST_DIR"))
}

/// `path` relative to the repository, as text.
fn relative(path: &Path) -> String {
    relative_text(&path.display().to_string())
}

/// `text` with the repository root cut out of every path in it, so reports show `examples/...`.
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

/// An example fails with one or more reports.
type failure = Vec<report>;

fn check_fails(f: &Path, out: &Path) -> Result<(), failure> {
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
    if got == want {
        return Ok(());
    }
    let text = std::fs::read_to_string(f).unwrap_or_default();
    let mut reports = Vec::new();
    for (line, code) in want.difference(&got) {
        let source_line = text.lines().nth(*line as usize - 1).unwrap_or("");
        let at = source_line.find("//~ ERROR").unwrap_or(0);
        let reported: Vec<&str> =
            got.iter().filter(|(l, _)| l == line).map(|(_, c)| c.as_str()).collect();
        reports.push(
            report::error(format!("crust did not report {code}"))
                .at(relative(f), *line, source_line[..at].chars().count() as u32 + 1)
                .underline(source_line[at..].chars().count())
                .label(format!(
                    "this line is marked as an error, and crust did not report {code} here"
                ))
                .row("wanted", format!("{code} on line {line}"))
                .row(
                    "found",
                    if reported.is_empty() {
                        "no error on this line".to_string()
                    } else {
                        reported.join(", ")
                    },
                ),
        );
    }
    for (line, code) in got.difference(&want) {
        reports.push(
            report::error(format!("crust reported an unexpected {code}"))
                .at(relative(f), *line, 0)
                .row("wanted", "no error on this line (it has no `//~ ERROR` marker)")
                .row("found", code.as_str())
                .help(format!("if the error is intended, end the line with `//~ ERROR {code}`")),
        );
    }
    let human = Command::new(crust()).arg(f).arg("-o").arg(out.join("x")).output().unwrap();
    reports.push(report::error(format!("what crust printed for {}", relative(f))).text_block(
        "crust",
        relative_text(String::from_utf8_lossy(&human.stderr).trim_end()).as_str(),
    ));
    Err(reports)
}

fn run(cmd: &mut Command) -> Result<String, failure> {
    let o = cmd.output().unwrap();
    if !o.status.success() {
        return Err(vec![
            report::error("a command in the example failed")
                .row("command", relative_text(&format!("{cmd:?}")))
                .row("wanted", "exit status 0")
                .process(&relative_output(&o)),
        ]);
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

fn expect(path: &Path, got: &str) -> Result<(), failure> {
    match std::fs::read_to_string(path) {
        Ok(want) if want == got => Ok(()),
        Ok(want) => Err(vec![
            report::error(format!("{} does not match what the example prints", relative(path)))
                .at(relative(path), 1, 0)
                .row(
                    "wanted",
                    format!("the contents of {}", path.file_name().unwrap().to_string_lossy()),
                )
                .row("found", format!("{} bytes on stdout", got.len()))
                .text_diff(&want, got)
                .help("if the new output is intended, update the .stdout file"),
        ]),
        Err(_) => Err(vec![
            report::error(format!("{} is missing", relative(path)))
                .row("wanted", "a file with the output the example prints")
                .help("create it from the example's stdout"),
        ]),
    }
}

fn check_example(dir: &Path) -> Result<(), failure> {
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
    let failures: Vec<(String, failure)> = dirs
        .iter()
        .filter_map(|d| check_example(d).err().map(|reports| (relative(d), reports)))
        .collect();
    if !failures.is_empty() {
        let mut text = String::new();
        for (_, reports) in &failures {
            for r in reports {
                text.push_str(&r.render());
                text.push_str("\n\n");
            }
        }
        let names: Vec<String> = failures.iter().map(|(n, _)| n.clone()).collect();
        let summary =
            report::error(format!("{} of {} examples failed", failures.len(), dirs.len()))
                .text_block("failed examples", &names.join("\n"));
        panic!("\n{text}{}", summary.render());
    }
}

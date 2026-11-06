/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
mod build;
mod crust;
mod git;
mod incremental;
mod lockfile;
mod manifest;
mod project;
mod resolve;
mod scan;
mod toml;
mod util;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use build::{BuildMode::*, TargetFilter::*, built_target};
use manifest::{TargetKind::*, package_manifest};
use project::AddSource::*;
use resolve::{ResolveMode::*, package_set};
use util::{absolute, crepe_error, crepe_result, status};

const Version: &str = env!("CARGO_PKG_VERSION");

const Usage: &str = "\
crepe: the Crust package manager

Usage: crepe <command> [options]

Commands:
    new <name>          Create a package in a new directory
    init                Create a package in the current directory
    add <name>          Add a dependency (--path <dir> | --git <url> [--rev <rev>])
    remove <name>       Remove a dependency
    build               Build the package and its dependencies
    run                 Build and run a binary
    test                Build and run the [[test]] targets
    check               Check the package without linking
    fetch               Download the dependencies named in the lockfile
    update [<name>]     Re-resolve dependencies and rewrite crepe.lock
    tree                Print the dependency tree
    clean               Remove build/

Options:
    -h, --help          Print help (also `crepe <command> --help`)
    -V, --version       Print the version

Environment:
    CRUST               The crust compiler to use
    CRUST_HOME          Where crepe keeps its cache (default ~/.crepe)
";

fn command_usage(command: &str) -> &'static str {
    match command {
        "new" => "crepe new <name>",
        "init" => "crepe init",
        "add" => "crepe add <name> [--path <dir> | --git <url> [--rev <rev>]]",
        "remove" => "crepe remove <name>",
        "build" => "crepe build [--release] [--target <name>]",
        "run" => "crepe run [--release] [--target <name>] [-- <args>...]",
        "test" => "crepe test",
        "check" => "crepe check",
        "fetch" => "crepe fetch",
        "update" => "crepe update [<name>]",
        "tree" => "crepe tree",
        "clean" => "crepe clean",
        _ => "crepe <command> [options]",
    }
}

/** A parsed command line for one command. */
#[derive(Debug, Default)]
struct cli_args {
    positional: Vec<String>,
    release: bool,
    target: Option<String>,
    path: Option<String>,
    git: Option<String>,
    rev: Option<String>,
    /** Arguments after `--`, for `crepe run`. */
    trailing: Vec<String>,
    help: bool,
}

/** Options each command accepts. `--` is accepted only by `run`. */
fn allowed_options(command: &str) -> &'static [&'static str] {
    match command {
        "build" => &["--release", "--target"],
        "run" => &["--release", "--target", "--"],
        "add" => &["--path", "--git", "--rev"],
        _ => &[],
    }
}

fn usage_error(command: &str, message: String) -> crepe_error {
    crepe_error::usage(format!(
        "{message}\n\nUsage: {}\n\nFor more information, try `--help`.",
        command_usage(command)
    ))
}

fn parse_args(command: &str, args: &[String]) -> crepe_result<cli_args> {
    let allowed = allowed_options(command);
    let mut out = cli_args::default();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        if arg == "-h" || arg == "--help" {
            out.help = true;
            continue;
        }
        if !arg.starts_with('-') || arg == "-" {
            out.positional.push(arg.clone());
            continue;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        if !allowed.contains(&flag.as_str()) {
            return Err(usage_error(
                command,
                format!("unexpected argument `{arg}` for `crepe {command}`"),
            ));
        }
        match flag.as_str() {
            "--" => {
                out.trailing = args[i..].to_vec();
                break;
            }
            "--release" if inline.is_none() => out.release = true,
            "--target" | "--path" | "--git" | "--rev" => {
                let value = match inline {
                    Some(v) => v,
                    None if i < args.len() => {
                        i += 1;
                        args[i - 1].clone()
                    }
                    None => return Err(usage_error(command, format!("`{flag}` needs a value"))),
                };
                let slot = match flag.as_str() {
                    "--target" => &mut out.target,
                    "--path" => &mut out.path,
                    "--git" => &mut out.git,
                    _ => &mut out.rev,
                };
                if slot.replace(value).is_some() {
                    return Err(usage_error(command, format!("`{flag}` was given twice")));
                }
            }
            _ => {
                return Err(usage_error(
                    command,
                    format!("unexpected argument `{arg}` for `crepe {command}`"),
                ));
            }
        }
    }
    Ok(out)
}

fn expect_positional(command: &str, args: &cli_args, min: usize, max: usize) -> crepe_result<()> {
    let n = args.positional.len();
    if n < min {
        return Err(usage_error(command, format!("`crepe {command}` needs more arguments")));
    }
    if n > max {
        return Err(usage_error(
            command,
            format!("unexpected argument `{}` for `crepe {command}`", args.positional[max]),
        ));
    }
    Ok(())
}

fn package_root() -> crepe_result<PathBuf> {
    manifest::find_package_root(&absolute(Path::new("."))?)
}

fn load_root() -> crepe_result<package_manifest> {
    manifest::load(&package_root()?)
}

fn resolve_root(mode: &resolve::ResolveMode) -> crepe_result<package_set> {
    resolve::resolve(load_root()?, mode)
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

fn run_command(command: &str, args: &[String]) -> crepe_result<i32> {
    let a = parse_args(command, args)?;
    if a.help {
        println!("Usage: {}", command_usage(command));
        return Ok(0);
    }
    match command {
        "new" => {
            expect_positional(command, &a, 1, 1)?;
            project::new_package(&a.positional[0])?;
        }
        "init" => {
            expect_positional(command, &a, 0, 0)?;
            project::init_package()?;
        }
        "add" => {
            expect_positional(command, &a, 1, 1)?;
            cmd_add(&a)?;
        }
        "remove" => {
            expect_positional(command, &a, 1, 1)?;
            project::remove_dependency(&package_root()?, &a.positional[0])?;
        }
        "build" => {
            expect_positional(command, &a, 0, 0)?;
            let set = resolve_root(&ResolveModeLocked)?;
            let filter = a.target.clone().map_or(TargetFilterDefault, TargetFilterNamed);
            build::build(&set, &filter, a.release, BuildModeBuild)?;
        }
        "run" => {
            expect_positional(command, &a, 0, 0)?;
            return cmd_run(&a);
        }
        "test" => {
            expect_positional(command, &a, 0, 0)?;
            return cmd_test();
        }
        "check" => {
            expect_positional(command, &a, 0, 0)?;
            let set = resolve_root(&ResolveModeLocked)?;
            build::build(&set, &TargetFilterAll, false, BuildModeCheck)?;
        }
        "fetch" => {
            expect_positional(command, &a, 0, 0)?;
            let set = resolve_root(&ResolveModeLocked)?;
            status("Fetched", &format!("{} dependencies", set.deps.len()));
        }
        "update" => {
            expect_positional(command, &a, 0, 1)?;
            resolve_root(&ResolveModeUpdate(a.positional.first().cloned()))?;
        }
        "tree" => {
            expect_positional(command, &a, 0, 0)?;
            let set = resolve_root(&ResolveModeLocked)?;
            print!("{}", resolve::format_tree(&set));
        }
        "clean" => {
            expect_positional(command, &a, 0, 0)?;
            cmd_clean()?;
        }
        _ => unreachable!("dispatch checked the command name"),
    }
    Ok(0)
}

fn cmd_add(a: &cli_args) -> crepe_result<()> {
    let spec = &a.positional[0];
    let (name, version) = match spec.split_once('@') {
        Some((n, v)) => (n.to_string(), Some(v.to_string())),
        None => (spec.clone(), None),
    };
    if a.rev.is_some() && a.git.is_none() {
        return Err(usage_error("add", "`--rev` needs `--git`".to_string()));
    }
    let source = match (&a.path, &a.git, version) {
        (Some(_), Some(_), _) => {
            return Err(usage_error("add", "use `--path` or `--git`, not both".to_string()));
        }
        (Some(_), _, Some(_)) | (_, Some(_), Some(_)) => {
            return Err(usage_error(
                "add",
                "a version (`@...`) cannot be combined with `--path` or `--git`".to_string(),
            ));
        }
        (Some(p), None, None) => AddSourcePath(p.clone()),
        (None, Some(g), None) => AddSourceGit { url: g.clone(), rev: a.rev.clone() },
        (None, None, v) => AddSourceRegistry(v),
    };
    project::add_dependency(&package_root()?, &name, source)
}

fn pick_bin(set: &package_set, target: Option<&str>) -> crepe_result<String> {
    let bins: Vec<&str> = set
        .root
        .manifest
        .targets
        .iter()
        .filter(|t| t.kind == TargetKindBin)
        .map(|t| t.name.as_str())
        .collect();
    if let Some(name) = target {
        if bins.contains(&name) {
            return Ok(name.to_string());
        }
        return Err(crepe_error::new(format!(
            "no [[bin]] target named `{name}`; binaries: {}",
            list_or_none(&bins)
        )));
    }
    match bins.as_slice() {
        [one] => Ok(one.to_string()),
        [] => Err(crepe_error::new(format!(
            "package `{}` has no binary to run\nhelp: add `src/main.crm` or a [[bin]] table",
            set.root.name()
        ))),
        many => Err(crepe_error::new(format!(
            "several binaries; pick one with `--target`: {}",
            list_or_none(many)
        ))),
    }
}

fn list_or_none(names: &[&str]) -> String {
    if names.is_empty() {
        return "none".to_string();
    }
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

fn cmd_run(a: &cli_args) -> crepe_result<i32> {
    let set = resolve_root(&ResolveModeLocked)?;
    let name = pick_bin(&set, a.target.as_deref())?;
    let filter = TargetFilterExact(TargetKindBin, name);
    let built = build::build(&set, &filter, a.release, BuildModeBuild)?;
    let Some(bin) = built.first() else {
        return Err(crepe_error::new("nothing was built"));
    };
    status("Running", &format!("`{}`", util::display_path(set.root.dir(), &bin.output)));
    let child = Command::new(&bin.output)
        .args(&a.trailing)
        .status()
        .map_err(|e| crepe_error::new(format!("could not run `{}`: {e}", bin.output.display())))?;
    Ok(exit_code(child))
}

fn run_test(set: &package_set, test: &built_target) -> crepe_result<bool> {
    let output = Command::new(&test.output)
        .current_dir(set.root.dir())
        .output()
        .map_err(|e| crepe_error::new(format!("could not run `{}`: {e}", test.output.display())))?;
    let ok = output.status.success();
    println!("test {} ... {}", test.target.name, if ok { "ok" } else { "FAILED" });
    if !ok {
        println!(
            "---- {} stdout ----\n{}",
            test.target.name,
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "---- {} stderr ----\n{}",
            test.target.name,
            String::from_utf8_lossy(&output.stderr)
        );
        println!("---- {} exit status: {} ----", test.target.name, exit_code(output.status));
    }
    Ok(ok)
}

fn cmd_test() -> crepe_result<i32> {
    let set = resolve_root(&ResolveModeLocked)?;
    let built = build::build(&set, &TargetFilterTests, false, BuildModeBuild)?;
    if built.is_empty() {
        util::warn("no [[test]] targets in crepe.toml");
        return Ok(0);
    }
    println!("\nrunning {} test(s)", built.len());
    let mut failed = Vec::new();
    for test in &built {
        if !run_test(&set, test)? {
            failed.push(test.target.name.clone());
        }
    }
    let passed = built.len() - failed.len();
    let verdict = if failed.is_empty() { "ok" } else { "FAILED" };
    println!("\ntest result: {verdict}. {passed} passed; {} failed", failed.len());
    if failed.is_empty() {
        return Ok(0);
    }
    Err(crepe_error::new(format!("test failed: {}", failed.join(", "))))
}

fn cmd_clean() -> crepe_result<()> {
    let root = package_root()?;
    let build_dir = root.join("build");
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)
            .map_err(|e| util::io_error("remove", &build_dir, &e))?;
        status("Removed", &build_dir.display().to_string());
    }
    Ok(())
}

fn run(args: &[String]) -> crepe_result<i32> {
    let Some(first) = args.first() else {
        print!("{Usage}");
        return Ok(0);
    };
    match first.as_str() {
        "-h" | "--help" => {
            print!("{Usage}");
            Ok(0)
        }
        "-V" | "--version" => {
            println!("crepe {Version}");
            Ok(0)
        }
        "new" | "init" | "add" | "remove" | "build" | "run" | "test" | "check" | "fetch"
        | "update" | "tree" | "clean" => run_command(first, &args[1..]),
        other => Err(crepe_error::usage(format!(
            "unknown command `{other}`\nhelp: run `crepe --help` for the list of commands"
        ))),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(code) => code,
        Err(e) => {
            for message in &e.messages {
                eprintln!("error: {message}");
            }
            e.exit_code
        }
    };
    std::process::exit(code);
}

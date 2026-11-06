/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use crate::util::{crepe_error, crepe_result};

/** Exit codes from docs/CLI.md, section 1. */
const CrustExitCompileError: i32 = 1;
const CrustExitUsage: i32 = 2;
const CrustExitIce: i32 = 70;

/** The compiler crepe drives. */
#[derive(Clone, Debug)]
pub struct crust_tool {
    pub program: PathBuf,
    /** Path, size, and modification time of the binary. A new compiler rebuilds everything. */
    pub identity: String,
}

fn exe_name(base: &str) -> String {
    format!("{base}{}", env::consts::EXE_SUFFIX)
}

fn search_path(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths).map(|dir| dir.join(name)).find(|p| p.is_file())
}

/** `$CRUST`, else `crust` next to the running crepe, else `crust` on `PATH`. */
pub fn find_crust() -> crust_tool {
    let program = env::var_os("CRUST")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let exe = env::current_exe().ok()?;
            let sibling = exe.parent()?.join(exe_name("crust"));
            sibling.is_file().then_some(sibling)
        })
        .or_else(|| search_path(&exe_name("crust")))
        .unwrap_or_else(|| PathBuf::from("crust"));
    let identity = match std::fs::metadata(&program) {
        Ok(meta) => {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos());
            format!("{} {} {}", program.display(), meta.len(), mtime.unwrap_or(0))
        }
        Err(_) => program.display().to_string(),
    };
    crust_tool { program, identity }
}

fn shell_words(program: &Path, args: &[String]) -> String {
    let mut out = program.display().to_string();
    for arg in args {
        out.push(' ');
        if arg.is_empty() || arg.contains(|c: char| c.is_whitespace() || "'\"\\$".contains(c)) {
            out.push_str(&format!("'{}'", arg.replace('\'', "'\\''")));
        } else {
            out.push_str(arg);
        }
    }
    out
}

impl crust_tool {
    /**
     * Runs crust in `cwd`. Diagnostics go straight to the terminal; `what` names the
     * thing being built in the error that follows them.
     */
    pub fn run(&self, cwd: &Path, args: &[String], what: &str) -> crepe_result<()> {
        let status =
            Command::new(&self.program).args(args).current_dir(cwd).status().map_err(|e| {
                crepe_error::new(format!(
                    "could not run the compiler `{}`: {e}\n\
                 help: put `crust` on PATH, or set CRUST to its path",
                    self.program.display()
                ))
            })?;
        let command = shell_words(&self.program, args);
        match status.code() {
            Some(0) => Ok(()),
            Some(CrustExitCompileError) => {
                Err(crepe_error::new(format!("could not compile `{what}`")))
            }
            Some(CrustExitUsage) => Err(crepe_error::new(format!(
                "crust rejected the command line while building `{what}` (exit 2); this is a \
                 bug in crepe or a crust version mismatch\ncommand: {command}"
            ))),
            Some(CrustExitIce) => Err(crepe_error::new(format!(
                "internal compiler error in crust while building `{what}`; please report it\n\
                 command: {command}"
            ))),
            Some(code) => Err(crepe_error::new(format!(
                "crust exited with status {code} while building `{what}`\ncommand: {command}"
            ))),
            None => Err(crepe_error::new(format!(
                "crust was killed by a signal while building `{what}`\ncommand: {command}"
            ))),
        }
    }
}

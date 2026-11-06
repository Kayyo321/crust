/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The `crust` entry point: option parsing and pipeline wiring
// (docs/CLI.md, section 1). Only the driver prints and exits.
mod cli_options;
mod pipeline;
mod toolchain;

use crate::diag;
pub use cli_options::*;
use std::io::Write;

pub const UsageText: &str = "\
Usage: crust [options] <input>...

Inputs: .c (C23), .cr (Crust source), .crm (Crust module), and .o/.a/.so link inputs.

Output:
  -o <path>               Output path
  --emit=<kind>           exe (default), obj, asm, llvm-ir, staticlib, sharedlib, header, interface
  -c                      Same as --emit=obj
  --emit-header=<path>    Also write a C header for [[crust::c_abi]] exports
Language and safety:
  --std=c23|crust         Override the language mode chosen by extension
  --unchecked             Turn off safety checks (migration only; warns)
Modules:
  --extern <pkg>=<dir>    Directory with the .crmi interfaces of a dependency
  --print-interface <f>   Print a .crmi file in readable form
Code generation:
  -O0 -O1 -O2 -O3 -Os     Optimization level (default -O0)
  -g                      Debug build
  --target=<triple>       Target triple (x86_64-pc-linux-gnu)
  -fPIC                   Position-independent code
Paths and linking:
  -I <dir>, -isystem <dir>, -D <name>[=value], -U <name>, -L <dir>, -l <name>, -shared
Diagnostics:
  --explain <CRnnnn>      Explain a diagnostic code
  --error-format=human|json
  --version, --help
";

/// Runs the compiler. Returns the process exit code (docs/CLI.md).
pub fn run(args: Vec<String>) -> i32 {
    let opts = match cli_options::parse(&args) {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("crust: error: {msg}");
            eprintln!("Try `crust --help`.");
            return 2;
        }
    };
    if opts.help {
        print!("{UsageText}");
        return 0;
    }
    if opts.version {
        println!("crust {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    if let Some(code) = &opts.explain {
        return explain(code);
    }
    if let Some(f) = &opts.print_interface {
        return match crate::modules::interface::print_file(f) {
            Ok(s) => {
                print!("{s}");
                0
            }
            Err(e) => {
                eprintln!("crust: error: {e}");
                1
            }
        };
    }
    if opts.inputs.is_empty() {
        eprintln!("crust: error: no input files");
        return 2;
    }
    let json = opts.json;
    let inputs = opts.inputs.clone();
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pipeline::compile(&opts)));
    match result {
        Ok(code) => {
            flush_diags(json);
            code
        }
        Err(e) => {
            flush_diags(json);
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            eprintln!("\nerror: internal compiler error: {msg}");
            eprintln!("note: this is a bug in crust, not in your code.");
            eprintln!("note: please report it with the input files and this command line:");
            eprintln!("note:   crust {}", args.join(" "));
            let _ = inputs;
            70
        }
    }
}

pub fn flush_diags(json: bool) {
    let mut err = std::io::stderr().lock();
    for d in diag::take_all() {
        let s = if json { diag::render_json(&d) + "\n" } else { diag::render_human(&d) };
        let _ = err.write_all(s.as_bytes());
    }
}

fn explain(code: &str) -> i32 {
    match diag::codes::lookup(code) {
        Some(c) => {
            println!("{}: {}\n\n{}", c.code, c.title, c.explanation);
            0
        }
        None => {
            eprintln!("crust: error: `{code}` is not a Crust diagnostic code");
            2
        }
    }
}

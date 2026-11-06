/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Emit {
    EmitExe,
    EmitObj,
    EmitAsm,
    EmitLlvmIr,
    EmitStaticlib,
    EmitSharedlib,
    EmitHeader,
    EmitInterface,
}
pub use Emit::*;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StdMode {
    StdModeC23,
    StdModeCrust,
}
pub use StdMode::*;

#[derive(Clone, Debug)]
pub struct options {
    pub inputs: Vec<PathBuf>,
    pub output: Option<PathBuf>,
    pub emit: Emit,
    pub emit_header: Option<PathBuf>,
    pub std: Option<StdMode>,
    pub unchecked: bool,
    pub externs: Vec<(String, PathBuf)>,
    pub print_interface: Option<PathBuf>,
    pub opt: String,
    pub debug: bool,
    pub target: Option<String>,
    pub pic: bool,
    pub include_dirs: Vec<PathBuf>,
    pub system_dirs: Vec<PathBuf>,
    pub defines: Vec<(String, String)>,
    pub undefs: Vec<String>,
    pub lib_dirs: Vec<PathBuf>,
    pub libs: Vec<String>,
    pub explain: Option<String>,
    pub json: bool,
    pub help: bool,
    pub version: bool,
}

pub fn parse(args: &[String]) -> Result<options, String> {
    let mut o = options {
        inputs: vec![],
        output: None,
        emit: EmitExe,
        emit_header: None,
        std: None,
        unchecked: false,
        externs: vec![],
        print_interface: None,
        opt: "-O0".into(),
        debug: false,
        target: None,
        pic: false,
        include_dirs: vec![],
        system_dirs: vec![],
        defines: vec![],
        undefs: vec![],
        lib_dirs: vec![],
        libs: vec![],
        explain: None,
        json: false,
        help: false,
        version: false,
    };
    let mut i = 0;
    // A flag's value: joined (`-Ifoo`, `--x=foo`) or the next argument.
    let value = |i: &mut usize, a: &str, flag: &str| -> Result<String, String> {
        let rest = &a[flag.len()..];
        let rest = rest.strip_prefix('=').unwrap_or(rest);
        if !rest.is_empty() {
            return Ok(rest.to_string());
        }
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| format!("`{flag}` needs a value"))
    };
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--help" | "-h" => o.help = true,
            "--version" => o.version = true,
            "-c" => o.emit = EmitObj,
            "-g" => o.debug = true,
            "-fPIC" | "-fpic" => o.pic = true,
            "-shared" => o.emit = EmitSharedlib,
            "--unchecked" => o.unchecked = true,
            "-O0" | "-O1" | "-O2" | "-O3" | "-Os" => o.opt = a.to_string(),
            "-O" => o.opt = "-O1".into(),
            _ if a.starts_with("--emit-header") => {
                o.emit_header = Some(PathBuf::from(value(&mut i, a, "--emit-header")?))
            }
            _ if a.starts_with("--emit") => {
                let v = value(&mut i, a, "--emit")?;
                o.emit = match v.as_str() {
                    "exe" => EmitExe,
                    "obj" => EmitObj,
                    "asm" => EmitAsm,
                    "llvm-ir" => EmitLlvmIr,
                    "staticlib" => EmitStaticlib,
                    "sharedlib" => EmitSharedlib,
                    "header" => EmitHeader,
                    "interface" => EmitInterface,
                    _ => return Err(format!("unknown --emit kind `{v}`")),
                };
            }
            _ if a.starts_with("--std") => {
                let v = value(&mut i, a, "--std")?;
                o.std = Some(match v.as_str() {
                    "c23" => StdModeC23,
                    "crust" => StdModeCrust,
                    _ => return Err(format!("unknown --std value `{v}`; use `c23` or `crust`")),
                });
            }
            _ if a.starts_with("--extern") => {
                let v = value(&mut i, a, "--extern")?;
                let (k, d) = v.split_once('=').ok_or("`--extern` takes `<package>=<dir>`")?;
                o.externs.push((k.to_string(), PathBuf::from(d)));
            }
            _ if a.starts_with("--print-interface") => {
                o.print_interface = Some(PathBuf::from(value(&mut i, a, "--print-interface")?))
            }
            _ if a.starts_with("--target") => o.target = Some(value(&mut i, a, "--target")?),
            _ if a.starts_with("--explain") => o.explain = Some(value(&mut i, a, "--explain")?),
            _ if a.starts_with("--error-format") => {
                let v = value(&mut i, a, "--error-format")?;
                o.json = match v.as_str() {
                    "json" => true,
                    "human" => false,
                    _ => return Err(format!("unknown --error-format `{v}`")),
                };
            }
            _ if a.starts_with("-isystem") => {
                o.system_dirs.push(PathBuf::from(value(&mut i, a, "-isystem")?))
            }
            _ if a.starts_with("-I") => o.include_dirs.push(PathBuf::from(value(&mut i, a, "-I")?)),
            _ if a.starts_with("-D") => {
                let v = value(&mut i, a, "-D")?;
                let (k, val) = v.split_once('=').unwrap_or((&v, "1"));
                o.defines.push((k.to_string(), val.to_string()));
            }
            _ if a.starts_with("-U") => o.undefs.push(value(&mut i, a, "-U")?),
            _ if a.starts_with("-L") => o.lib_dirs.push(PathBuf::from(value(&mut i, a, "-L")?)),
            _ if a.starts_with("-l") => o.libs.push(value(&mut i, a, "-l")?),
            _ if a.starts_with("-o") => o.output = Some(PathBuf::from(value(&mut i, a, "-o")?)),
            _ if a.starts_with('-') && a.len() > 1 => return Err(format!("unknown option `{a}`")),
            _ => o.inputs.push(PathBuf::from(a)),
        }
        i += 1;
    }
    if let Some(t) = &o.target {
        if !t.starts_with("x86_64") || !t.contains("linux") {
            return Err(format!(
                "target `{t}` is not supported by Crust 0.1.0; the supported target is x86_64-pc-linux-gnu"
            ));
        }
    }
    Ok(o)
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::cli_options::*;
use super::toolchain;
use crate::diag::{self, FileKind, span};
use crate::lex::*;
use crate::modules::{self, module_env, module_interface};
use crate::parse::{self, import_oracle};
use crate::pp;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

struct unit {
    path: PathBuf,
    file: u32,
    toks: Vec<token>,
    module: Option<(String, span)>,
    imports: Vec<(String, span)>,
}

/// Resolves imports against this invocation's modules, the output
/// directory's `.crmi` files, and `--extern` directories.
struct invocation_env {
    done: RefCell<HashMap<String, Rc<module_interface>>>,
    cache: RefCell<HashMap<PathBuf, Rc<module_interface>>>,
    outdir: PathBuf,
    externs: Vec<(String, PathBuf)>,
}

impl invocation_env {
    fn load(&self, p: &Path) -> Result<Rc<module_interface>, String> {
        if let Some(i) = self.cache.borrow().get(p) {
            return Ok(i.clone());
        }
        let bytes = std::fs::read(p)
            .map_err(|e| format!("cannot read interface `{}`: {e}", p.display()))?;
        let iface = Rc::new(
            modules::interface::decode(&bytes)
                .map_err(|e| format!("bad interface file `{}`: {e}", p.display()))?,
        );
        self.cache.borrow_mut().insert(p.to_path_buf(), iface.clone());
        Ok(iface)
    }
}

impl module_env for invocation_env {
    fn resolve(&self, spec: &str) -> Result<Rc<module_interface>, String> {
        if let Some((pkg, name)) = spec.split_once('/') {
            let Some((_, dir)) = self.externs.iter().find(|(p, _)| p == pkg) else {
                return Err(format!(
                    "package `{pkg}` is not available; pass `--extern {pkg}=<dir>` or list it under [dependencies] in crepe.toml"
                ));
            };
            return self.load(&dir.join(format!("{name}.crmi")));
        }
        if let Some(i) = self.done.borrow().get(spec) {
            return Ok(i.clone());
        }
        let local = self.outdir.join(format!("{spec}.crmi"));
        if local.is_file() {
            return self.load(&local);
        }
        if let Some((_, dir)) = self.externs.iter().find(|(p, _)| p == spec) {
            return self.load(&dir.join(format!("{spec}.crmi")));
        }
        Err(format!("no module named `{spec}` is part of this build"))
    }
}

struct oracle<'a>(&'a invocation_env);

impl import_oracle for oracle<'_> {
    fn is_typedef(&self, spec: &str, name: sym) -> bool {
        self.0.resolve(spec).map(|i| i.is_typedef(name)).unwrap_or(false)
    }
}

fn scan_module_info(toks: &[token]) -> (Option<(String, span)>, Vec<(String, span)>) {
    let mut module = None;
    let mut imports = Vec::new();
    let mut i = 0;
    if toks.first().is_some_and(|t| t.is(SymModule)) {
        let mut name = String::new();
        let mut sp = toks[0].span;
        i = 1;
        while i < toks.len() && !toks[i].is(SymSemi) && toks[i].kind != TokenKindEof {
            name.push_str(toks[i].text.as_str());
            sp = toks[0].span.to(toks[i].span);
            i += 1;
        }
        module = Some((name, sp));
    }
    while i < toks.len() {
        let is_import = toks[i].is(SymImport);
        let is_export = toks[i].is(SymExport);
        if is_import || is_export {
            let mut j = i + 1;
            while j < toks.len() && !toks[j].is(SymSemi) && toks[j].kind != TokenKindEof {
                if is_export && toks[j].is(SymLBrace) && j != i + 1 {
                    break;
                }
                if toks[j].is_ident_named(SymFrom) {
                    if let Some(s) = toks.get(j + 1).filter(|t| t.kind == TokenKindString) {
                        let txt = s.text.as_str();
                        imports.push((txt[1..txt.len() - 1].to_string(), s.span));
                    }
                }
                j += 1;
            }
            i = j;
        }
        i += 1;
    }
    (module, imports)
}

fn unique_temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let mut n = 0u32;
    loop {
        let d = base.join(format!("crust-{}-{n}", std::process::id()));
        if std::fs::create_dir(&d).is_ok() {
            return d;
        }
        n += 1;
    }
}

struct compiled {
    path: PathBuf,
    module: Option<String>,
    ir: String,
    iface: Option<module_interface>,
    header: Option<String>,
}

pub fn compile(opts: &options) -> i32 {
    crate::sema::set_unchecked(opts.unchecked);
    let mut sources = Vec::new();
    let mut link_inputs = Vec::new();
    for p in &opts.inputs {
        match p.extension().and_then(|e| e.to_str()) {
            Some("c" | "cr" | "crm") => sources.push(p.clone()),
            Some("o" | "a" | "so") => link_inputs.push(p.clone()),
            Some("h" | "hr") => {
                eprintln!(
                    "crust: error: `{}` is a header; headers are included, not compiled",
                    p.display()
                );
                return 2;
            }
            _ => {
                eprintln!(
                    "crust: error: `{}`: unknown input file type (expected .c, .cr, .crm, .o, .a, or .so)",
                    p.display()
                );
                return 2;
            }
        }
    }
    let single = sources.len() == 1;
    if opts.output.is_some()
        && !single
        && matches!(opts.emit, EmitObj | EmitAsm | EmitLlvmIr | EmitInterface)
    {
        eprintln!(
            "crust: error: `-o` with several inputs needs --emit=exe, staticlib, or sharedlib"
        );
        return 2;
    }
    let outdir = match &opts.output {
        Some(o) => o
            .parent()
            .map(|p| if p.as_os_str().is_empty() { PathBuf::from(".") } else { p.to_path_buf() })
            .unwrap_or(PathBuf::from(".")),
        None => PathBuf::from("."),
    };
    let mut pp_opts = pp::pp_options {
        quote_dirs: vec![],
        include_dirs: opts.include_dirs.clone(),
        system_dirs: opts.system_dirs.clone(),
        defines: opts.defines.clone(),
        undefs: opts.undefs.clone(),
    };
    pp_opts.system_dirs.extend(pp::default_system_dirs());

    // Preprocess every source.
    let mut units = Vec::new();
    for p in &sources {
        let text = match std::fs::read(p) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                eprintln!("crust: error: cannot read `{}`: {e}", p.display());
                return 1;
            }
        };
        let mut kind = FileKind::from_path(p);
        match opts.std {
            Some(StdModeC23) if kind.is_crust() => kind = crate::diag::FileKindC,
            Some(StdModeCrust) if !kind.is_crust() => kind = crate::diag::FileKindCr,
            _ => {}
        }
        let id = diag::add_file(p.clone(), p.display().to_string(), Rc::from(text.as_str()), kind);
        if opts.unchecked {
            diag::warning(
                "CR1801",
                span { file: id, lo: 0, hi: 0 },
                format!("safety checks are off for `{}` (--unchecked)", p.display()),
            )
            .note("--unchecked is for migration only; the memory-safety guarantee does not cover this file");
        }
        let mut toks = pp::preprocessor::new(pp_opts.clone()).run_file(id);
        classify(&mut toks);
        let (module, imports) = scan_module_info(&toks);
        units.push(unit { path: p.clone(), file: id, toks, module, imports });
    }
    if diag::error_count() > 0 {
        return 1;
    }
    let order = match order_units(&units) {
        Some(o) => o,
        None => return 1,
    };
    let env = invocation_env {
        done: RefCell::new(HashMap::new()),
        cache: RefCell::new(HashMap::new()),
        outdir: outdir.clone(),
        externs: opts.externs.clone(),
    };
    let mut outputs: Vec<compiled> = Vec::new();
    for idx in order {
        let u = &mut units[idx];
        let errs_before = diag::error_count();
        let toks = std::mem::take(&mut u.toks);
        let tu = parse::parse_translation_unit(toks, u.file, &oracle(&env));
        if diag::error_count() > errs_before {
            return 1;
        }
        let out = crate::sema::analyze(&tu, &env);
        if diag::error_count() > errs_before {
            return 1;
        }
        let iface = out.module.map(|m| modules::interface::build(m, out.exports.clone()));
        if let (Some(m), Some(i)) = (&u.module, &iface) {
            env.done.borrow_mut().insert(m.0.clone(), Rc::new(i.clone()));
        }
        let mut bodies = Vec::new();
        for f in &out.prog.functions {
            let mut b = crate::crir::lower::lower_function(f, &out.prog);
            crate::safety::check_body(&mut b, &out.prog, !opts.unchecked);
            bodies.push(b);
        }
        if diag::error_count() > errs_before {
            return 1;
        }
        let ir = crate::codegen::emit_module(&out.prog, &bodies, &u.path.display().to_string());
        if diag::error_count() > errs_before {
            return 1;
        }
        let header = iface.as_ref().map(|i| modules::header::generate(i, &u.path));
        outputs.push(compiled {
            path: u.path.clone(),
            module: u.module.as_ref().map(|m| m.0.clone()),
            ir,
            iface,
            header,
        });
    }
    match write_outputs(opts, &outputs, &link_inputs, &outdir) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("crust: error: {e}");
            1
        }
    }
}

/// Orders the inputs so every module comes after the modules it imports.
fn order_units(units: &[unit]) -> Option<Vec<usize>> {
    let mut by_name: HashMap<&str, usize> = HashMap::new();
    let mut ok = true;
    for (i, u) in units.iter().enumerate() {
        if let Some((m, sp)) = &u.module {
            if let Some(&prev) = by_name.get(m.as_str()) {
                diag::error("CR1410", *sp, format!("module `{m}` is declared twice in this build"))
                    .secondary(units[prev].module.as_ref().unwrap().1, "first declared here");
                ok = false;
            }
            by_name.insert(m, i);
        }
    }
    if !ok {
        return None;
    }
    let mut state = vec![0u8; units.len()];
    let mut order = Vec::new();
    fn visit(
        i: usize,
        units: &[unit],
        by_name: &HashMap<&str, usize>,
        state: &mut [u8],
        order: &mut Vec<usize>,
        stack: &mut Vec<usize>,
    ) -> bool {
        if state[i] == 2 {
            return true;
        }
        if state[i] == 1 {
            let start = stack.iter().position(|&s| s == i).unwrap_or(0);
            let names: Vec<String> = stack[start..]
                .iter()
                .chain(std::iter::once(&i))
                .map(|&s| units[s].module.as_ref().map(|m| m.0.clone()).unwrap_or_default())
                .collect();
            let sp = units[i].module.as_ref().map(|m| m.1).unwrap_or_default();
            diag::error("CR1407", sp, format!("import cycle: {}", names.join(" -> ")))
                .help("move the shared declarations into a third module that both import");
            return false;
        }
        state[i] = 1;
        stack.push(i);
        for (spec, _) in &units[i].imports {
            if let Some(&j) = by_name.get(spec.as_str()) {
                if !visit(j, units, by_name, state, order, stack) {
                    return false;
                }
            }
        }
        stack.pop();
        state[i] = 2;
        order.push(i);
        true
    }
    for i in 0..units.len() {
        let mut stack = Vec::new();
        if !visit(i, units, &by_name, &mut state, &mut order, &mut stack) {
            return None;
        }
    }
    Some(order)
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into())
}

fn write_if_changed(p: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::read(p).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    std::fs::write(p, bytes).map_err(|e| format!("cannot write `{}`: {e}", p.display()))
}

fn write_outputs(
    opts: &options,
    outs: &[compiled],
    link_inputs: &[PathBuf],
    outdir: &Path,
) -> Result<(), String> {
    let single = outs.len() == 1;
    let tmp = unique_temp_dir();
    let result = (|| -> Result<(), String> {
        if let Some(h) = &opts.emit_header {
            let text: Vec<&str> = outs.iter().filter_map(|c| c.header.as_deref()).collect();
            if text.is_empty() {
                return Err("--emit-header needs a `.crm` module input".into());
            }
            std::fs::write(h, text.join("\n"))
                .map_err(|e| format!("cannot write `{}`: {e}", h.display()))?;
        }
        match opts.emit {
            EmitLlvmIr | EmitAsm | EmitObj => {
                for c in outs {
                    let ext = match opts.emit {
                        EmitLlvmIr => "ll",
                        EmitAsm => "s",
                        _ => "o",
                    };
                    let out = match (&opts.output, single) {
                        (Some(o), true) => o.clone(),
                        _ => PathBuf::from(format!("{}.{ext}", stem(&c.path))),
                    };
                    if opts.emit == EmitLlvmIr {
                        std::fs::write(&out, &c.ir)
                            .map_err(|e| format!("cannot write `{}`: {e}", out.display()))?;
                    } else {
                        let ll = tmp.join(format!("{}.ll", stem(&c.path)));
                        std::fs::write(&ll, &c.ir).map_err(|e| e.to_string())?;
                        toolchain::compile_ir(
                            &ll,
                            &out,
                            &opts.opt,
                            opts.pic,
                            opts.debug,
                            opts.emit == EmitAsm,
                        )?;
                    }
                    if let (Some(m), Some(i)) = (&c.module, &c.iface) {
                        let dir =
                            out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(outdir);
                        if out.as_os_str() != "/dev/null" {
                            write_if_changed(
                                &dir.join(format!("{m}.crmi")),
                                &crate::modules::interface::encode(i),
                            )?;
                        }
                    }
                }
                Ok(())
            }
            EmitInterface => {
                for c in outs {
                    let (Some(m), Some(i)) = (&c.module, &c.iface) else {
                        return Err(format!(
                            "`{}` is not a module; only `.crm` files have interfaces",
                            c.path.display()
                        ));
                    };
                    let out = match (&opts.output, single) {
                        (Some(o), true) => o.clone(),
                        _ => PathBuf::from(format!("{m}.crmi")),
                    };
                    write_if_changed(&out, &crate::modules::interface::encode(i))?;
                }
                Ok(())
            }
            EmitHeader => {
                let text: Vec<&str> = outs.iter().filter_map(|c| c.header.as_deref()).collect();
                if text.is_empty() {
                    return Err("--emit=header needs a `.crm` module input".into());
                }
                let out = opts.output.clone().unwrap_or_else(|| {
                    PathBuf::from(format!("{}.h", outs[0].module.clone().unwrap_or_default()))
                });
                std::fs::write(&out, text.join("\n"))
                    .map_err(|e| format!("cannot write `{}`: {e}", out.display()))
            }
            EmitExe | EmitStaticlib | EmitSharedlib => {
                let mut objs = Vec::new();
                let pic = opts.pic || opts.emit == EmitSharedlib;
                for (k, c) in outs.iter().enumerate() {
                    let ll = tmp.join(format!("{k}-{}.ll", stem(&c.path)));
                    let o = tmp.join(format!("{k}-{}.o", stem(&c.path)));
                    std::fs::write(&ll, &c.ir).map_err(|e| e.to_string())?;
                    toolchain::compile_ir(&ll, &o, &opts.opt, pic, opts.debug, false)?;
                    objs.push(o);
                }
                objs.extend(link_inputs.iter().cloned());
                let first = outs.first().map(|c| stem(&c.path)).unwrap_or_else(|| "out".into());
                match opts.emit {
                    EmitExe => {
                        let out = opts.output.clone().unwrap_or_else(|| PathBuf::from("a.out"));
                        toolchain::link(&objs, &out, &opts.lib_dirs, &opts.libs, false)
                    }
                    EmitStaticlib => {
                        let out = opts
                            .output
                            .clone()
                            .unwrap_or_else(|| PathBuf::from(format!("lib{first}.a")));
                        toolchain::archive(&objs, &out)
                    }
                    _ => {
                        let out = opts
                            .output
                            .clone()
                            .unwrap_or_else(|| PathBuf::from(format!("lib{first}.so")));
                        toolchain::link(&objs, &out, &opts.lib_dirs, &opts.libs, true)
                    }
                }
            }
        }
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

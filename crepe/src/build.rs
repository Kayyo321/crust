/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::crust::{crust_tool, find_crust};
use crate::incremental::{
    format_state, hash_source, load_state, module_state, read_interface_hash, stale_reason,
};
use crate::manifest::{TargetKind, TargetKind::*, build_target};
use crate::resolve::{package, package_set};
use crate::scan::{self, ImportTarget, ImportTarget::*, module_graph, source_file};
use crate::util::{
    create_dir_all, crepe_error, crepe_result, display_path, fnv_hasher, hex64, normalize, status,
    warn, write_file,
};

/** Which of the root package's targets to build. */
// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq)]
pub enum TargetFilter {
    /** Every `[[bin]]` and `[[lib]]` target: what `crepe build` builds. */
    TargetFilterDefault,
    /** Every target, tests included: what `crepe check` checks. */
    TargetFilterAll,
    /** Every target with this name. */
    TargetFilterNamed(String),
    /** The one target of this kind and name. */
    TargetFilterExact(TargetKind, String),
    /** Every `[[test]]` target: what `crepe test` builds. */
    TargetFilterTests,
}

pub use TargetFilter::*;

// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildMode {
    BuildModeBuild,
    /** Compile modules (importers need their interfaces) but link nothing. */
    BuildModeCheck,
}

pub use BuildMode::*;

/** A target that was built, and where its output is. */
#[derive(Clone, Debug)]
pub struct built_target {
    pub target: build_target,
    pub output: PathBuf,
}

pub fn profile_name(release: bool) -> &'static str {
    if release { "release" } else { "debug" }
}

pub fn profile_flags(release: bool) -> Vec<String> {
    let flags: &[&str] = if release { &["-O2"] } else { &["-O0", "-g"] };
    flags.iter().map(|f| f.to_string()).collect()
}

/** A `.cr` source of a target, with its resolved imports. */
struct cr_source {
    file: source_file,
    imports: Vec<ImportTarget>,
}

/** One package's share of the build. */
struct package_plan<'a> {
    pkg: &'a package,
    graph: module_graph,
    /** `build/<profile>` for the root package, `build/<profile>/deps/<name>` for a dependency. */
    out_dir: PathBuf,
    /** Modules to compile, each after the modules it imports. */
    order: Vec<usize>,
}

impl package_plan<'_> {
    fn modules_dir(&self) -> PathBuf {
        self.out_dir.join("modules")
    }
}

/** A selected target of the root package. */
struct target_plan {
    target: build_target,
    root_module: Option<usize>,
    sources: Vec<cr_source>,
    /** Same-package modules the target links, in dependency order. */
    modules: Vec<usize>,
}

struct session<'a> {
    set: &'a package_set,
    crust: crust_tool,
    release: bool,
    mode: BuildMode,
    /** `<root>/build/<profile>`. */
    build_dir: PathBuf,
}

/**
 * Builds (or checks) the selected targets of the root package and the dependencies they
 * need, recompiling only stale modules (D-030).
 */
pub fn build(
    set: &package_set,
    filter: &TargetFilter,
    release: bool,
    mode: BuildMode,
) -> crepe_result<Vec<built_target>> {
    let s = session {
        set,
        crust: find_crust(),
        release,
        mode,
        build_dir: set.root.dir().join("build").join(profile_name(release)),
    };
    let targets = select_targets(set, filter)?;
    let mut plans = load_graphs(set, &s.build_dir)?;
    let mut target_plans = Vec::new();
    for target in targets {
        target_plans.push(plan_target(&plans[0], target)?);
    }
    plan_modules(&mut plans, &target_plans)?;
    let externs = extern_dirs(&plans);
    for plan in plans.iter().skip(1) {
        s.build_dependency(plan, &externs)?;
    }
    for &index in &plans[0].order {
        s.compile_module(&plans[0], index, &externs)?;
    }
    let dep_libs: Vec<PathBuf> = plans
        .iter()
        .skip(1)
        .rev()
        .map(|p| lib_path(&p.out_dir, p.pkg.name()))
        .filter(|p| p.is_file())
        .collect();
    let mut built = Vec::new();
    for tp in &target_plans {
        built.push(s.finish_target(&plans[0], tp, &externs, &dep_libs)?);
    }
    status("Finished", &format!("{} target(s)", profile_name(release)));
    Ok(built)
}

fn select_targets(set: &package_set, filter: &TargetFilter) -> crepe_result<Vec<build_target>> {
    let all = &set.root.manifest.targets;
    let picked: Vec<build_target> = all
        .iter()
        .filter(|t| match filter {
            TargetFilterDefault => t.kind != TargetKindTest,
            TargetFilterAll => true,
            TargetFilterNamed(n) => &t.name == n,
            TargetFilterExact(k, n) => t.kind == *k && &t.name == n,
            TargetFilterTests => t.kind == TargetKindTest,
        })
        .cloned()
        .collect();
    if !picked.is_empty() || *filter == TargetFilterTests {
        return Ok(picked);
    }
    if let TargetFilterNamed(n) | TargetFilterExact(_, n) = filter {
        let names: Vec<_> = all.iter().map(|t| format!("`{}`", t.name)).collect();
        let list = if names.is_empty() { "none".to_string() } else { names.join(", ") };
        return Err(crepe_error::new(format!(
            "no target named `{n}` in package `{}`; targets: {list}",
            set.root.name()
        )));
    }
    Err(crepe_error::new(format!(
        "package `{}` has no targets\nhelp: add `src/main.crm` or `src/lib.crm`, or declare \
         [[bin]] or [[lib]] in crepe.toml",
        set.root.name()
    )))
}

/** Scans every package and builds its module graph. The root package comes first. */
fn load_graphs<'a>(
    set: &'a package_set,
    profile_dir: &Path,
) -> crepe_result<Vec<package_plan<'a>>> {
    let mut plans = Vec::new();
    let mut errors = Vec::new();
    let mut packages = vec![&set.root];
    packages.extend(set.order.iter().map(|n| &set.deps[n]));
    for (i, pkg) in packages.into_iter().enumerate() {
        let out_dir = if i == 0 {
            profile_dir.to_path_buf()
        } else {
            profile_dir.join("deps").join(pkg.name())
        };
        match package_graph(pkg) {
            Ok(graph) => plans.push(package_plan { pkg, graph, out_dir, order: Vec::new() }),
            Err(errs) => errors.extend(errs),
        }
    }
    if !errors.is_empty() {
        return Err(crepe_error::many(errors));
    }
    Ok(plans)
}

/** Every `.crm` under `src/`, plus target roots that live elsewhere (such as `tests/`). */
fn package_graph(pkg: &package) -> Result<module_graph, Vec<String>> {
    let dir = pkg.dir();
    let mut paths = scan::find_modules(&dir.join("src"));
    for t in &pkg.manifest.targets {
        if let Some(root) = &t.root {
            let abs = normalize(&dir.join(root));
            if abs.is_file() && !paths.contains(&abs) {
                paths.push(abs);
            }
        }
    }
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for p in paths {
        match scan::read_source(dir, &p) {
            Ok(f) => files.push(f),
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let deps: BTreeSet<String> = pkg.dep_names().into_iter().collect();
    let (graph, warnings) = scan::build_graph(pkg.name(), dir, files, &deps).map_err(|errs| {
        errs.into_iter().map(|e| format!("package `{}`: {e}", pkg.name())).collect::<Vec<_>>()
    })?;
    for w in warnings {
        warn(&format!("package `{}`: {w}", pkg.name()));
    }
    Ok(graph)
}

fn root_module_of(plan: &package_plan, target: &build_target) -> crepe_result<Option<usize>> {
    let Some(root) = &target.root else {
        return Ok(None);
    };
    let abs = normalize(&plan.pkg.dir().join(root));
    match plan.graph.by_path.get(&abs) {
        Some(&i) => Ok(Some(i)),
        None => Err(crepe_error::new(format!(
            "package `{}`: [[{}]] `{}`: root `{}` does not exist",
            plan.pkg.name(),
            target.kind.table_name(),
            target.name,
            root.display()
        ))),
    }
}

fn read_cr_sources(plan: &package_plan, target: &build_target) -> crepe_result<Vec<cr_source>> {
    let local: BTreeSet<String> = plan.graph.by_name.keys().cloned().collect();
    let deps: BTreeSet<String> = plan.pkg.dep_names().into_iter().collect();
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for rel in &target.sources {
        let abs = normalize(&plan.pkg.dir().join(rel));
        let file = match scan::read_source(plan.pkg.dir(), &abs) {
            Ok(f) => f,
            Err(e) => {
                errors.push(format!("[[{}]] `{}`: {e}", target.kind.table_name(), target.name));
                continue;
            }
        };
        let (imports, errs) = scan::resolve_imports(&file, &local, &deps);
        errors.extend(errs);
        out.push(cr_source { file, imports });
    }
    if !errors.is_empty() {
        return Err(crepe_error::many(errors));
    }
    Ok(out)
}

fn local_index(graph: &module_graph, imports: &[ImportTarget]) -> Vec<usize> {
    imports
        .iter()
        .filter_map(|i| match i {
            ImportTargetLocal(m) => graph.by_name.get(m).copied(),
            ImportTargetExtern { .. } => None,
        })
        .collect()
}

fn plan_target(plan: &package_plan, target: build_target) -> crepe_result<target_plan> {
    let root_module = root_module_of(plan, &target)?;
    let sources = read_cr_sources(plan, &target)?;
    let mut roots: Vec<usize> = root_module.into_iter().collect();
    for src in &sources {
        roots.extend(local_index(&plan.graph, &src.imports));
    }
    let modules = scan::dependency_order(&plan.graph, &roots);
    Ok(target_plan { target, root_module, sources, modules })
}

/** Records the modules of `package` that `imports` names. */
fn request(needs: &mut BTreeMap<String, BTreeSet<String>>, imports: &[ImportTarget]) {
    for i in imports {
        if let ImportTargetExtern { package, module } = i {
            needs.entry(package.clone()).or_default().insert(module.clone());
        }
    }
}

/**
 * Decides which modules each package compiles: the root package compiles what its
 * targets reach; a dependency compiles its library plus whatever its dependents import.
 */
fn plan_modules(plans: &mut [package_plan], targets: &[target_plan]) -> crepe_result<()> {
    let mut roots: Vec<usize> = Vec::new();
    let mut needs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for tp in targets {
        roots.extend(&tp.modules);
        for src in &tp.sources {
            request(&mut needs, &src.imports);
        }
    }
    plans[0].order = scan::dependency_order(&plans[0].graph, &roots);
    for &i in &plans[0].order {
        request(&mut needs, &plans[0].graph.modules[i].imports);
    }
    // Dependents come before their dependencies in reverse order.
    for p in (1..plans.len()).rev() {
        let plan = &plans[p];
        let wanted = needs.remove(plan.pkg.name()).unwrap_or_default();
        let (order, more) = plan_dependency(plan, &wanted)?;
        for (pkg, modules) in more {
            needs.entry(pkg).or_default().extend(modules);
        }
        plans[p].order = order;
    }
    Ok(())
}

fn plan_dependency(
    plan: &package_plan,
    wanted: &BTreeSet<String>,
) -> crepe_result<(Vec<usize>, BTreeMap<String, BTreeSet<String>>)> {
    let name = plan.pkg.name();
    let lib = plan.pkg.manifest.lib_target();
    let Some(lib) = lib else {
        if wanted.is_empty() {
            warn(&format!("dependency `{name}` has no [[lib]] target; nothing to build"));
            return Ok((Vec::new(), BTreeMap::new()));
        }
        return Err(crepe_error::new(format!(
            "dependency `{name}` has no [[lib]] target, but its modules are imported"
        )));
    };
    let mut roots: Vec<usize> = root_module_of(plan, lib)?.into_iter().collect();
    let mut errors = Vec::new();
    for module in wanted {
        match plan.graph.by_name.get(module) {
            Some(&i) => roots.push(i),
            None => errors.push(format!("package `{name}` has no module named `{module}`")),
        }
    }
    let mut more = BTreeMap::new();
    for src in read_cr_sources(plan, lib)? {
        roots.extend(local_index(&plan.graph, &src.imports));
        request(&mut more, &src.imports);
    }
    if !errors.is_empty() {
        return Err(crepe_error::many(errors));
    }
    let order = scan::dependency_order(&plan.graph, &roots);
    for &i in &order {
        request(&mut more, &plan.graph.modules[i].imports);
    }
    Ok((order, more))
}

/** Package name to the directory holding its `.crmi` files, for `--extern`. */
fn extern_dirs(plans: &[package_plan]) -> BTreeMap<String, PathBuf> {
    plans.iter().skip(1).map(|p| (p.pkg.name().to_string(), p.modules_dir())).collect()
}

fn lib_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("lib{name}.a"))
}

fn extern_args(imports: &[ImportTarget], externs: &BTreeMap<String, PathBuf>) -> Vec<String> {
    let packages: BTreeSet<&String> = imports
        .iter()
        .filter_map(|i| match i {
            ImportTargetExtern { package, .. } => Some(package),
            ImportTargetLocal(_) => None,
        })
        .collect();
    let mut args = Vec::new();
    for p in packages {
        if let Some(dir) = externs.get(p) {
            args.push("--extern".to_string());
            args.push(format!("{p}={}", dir.display()));
        }
    }
    args
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/**
 * True when `output` must be (re)linked: it is missing, an input is newer, or the
 * command changed since the stamp was written.
 */
fn link_needed(output: &Path, inputs: &[PathBuf], fingerprint: u64, stamp: &Path) -> bool {
    let Some(out_time) = modified(output) else {
        return true;
    };
    if fs::read_to_string(stamp).ok().as_deref() != Some(hex64(fingerprint).as_str()) {
        return true;
    }
    inputs.iter().any(|i| modified(i).is_none_or(|t| t > out_time))
}

fn fingerprint(identity: &str, args: &[String]) -> u64 {
    let mut h = fnv_hasher::new();
    h.field(identity);
    for a in args {
        h.field(a);
    }
    h.finish()
}

impl session<'_> {
    fn display_module(&self, plan: &package_plan, index: usize) -> String {
        let node = &plan.graph.modules[index];
        let path = display_path(self.set.root.dir(), &node.file.path);
        if std::ptr::eq(plan.pkg, &self.set.root) {
            format!("{} ({path})", node.name)
        } else {
            format!("{}/{} ({path})", plan.pkg.name(), node.name)
        }
    }

    fn interface_hash(
        &self,
        plan: &package_plan,
        target: &ImportTarget,
        externs: &BTreeMap<String, PathBuf>,
    ) -> crepe_result<u64> {
        let path = match target {
            ImportTargetLocal(m) => plan.modules_dir().join(format!("{m}.crmi")),
            ImportTargetExtern { package, module } => {
                let dir = externs.get(package).cloned().unwrap_or_default();
                dir.join(format!("{module}.crmi"))
            }
        };
        if !path.is_file() {
            return Err(crepe_error::new(format!(
                "interface for `{}` not found at `{}`; crust did not write it",
                target.key(),
                path.display()
            )));
        }
        read_interface_hash(&path)
    }

    /** Compiles one module unless its state says it is fresh (D-030). */
    fn compile_module(
        &self,
        plan: &package_plan,
        index: usize,
        externs: &BTreeMap<String, PathBuf>,
    ) -> crepe_result<()> {
        let node = &plan.graph.modules[index];
        let dir = plan.modules_dir();
        create_dir_all(&dir)?;
        let object = dir.join(format!("{}.o", node.name));
        let interface = dir.join(format!("{}.crmi", node.name));
        let state_path = dir.join(format!("{}.state", node.name));
        let mut args = vec![
            "-c".to_string(),
            node.file.rel.clone(),
            "-o".to_string(),
            object.display().to_string(),
        ];
        args.extend(profile_flags(self.release));
        args.extend(extern_args(&node.imports, externs));
        let mut imports = Vec::new();
        for target in &node.imports {
            imports.push((target.key(), self.interface_hash(plan, target, externs)?));
        }
        imports.sort();
        let includes_of = |text: &str| scan::scan_source(text).includes;
        let current = module_state {
            source_hash: hash_source(&node.file.path, &includes_of)?,
            flags_hash: fingerprint(&self.crust.identity, &args),
            imports,
        };
        let previous = load_state(&state_path);
        let outputs_exist = object.is_file() && interface.is_file();
        if stale_reason(previous.as_ref(), &current, outputs_exist).is_none() {
            return Ok(());
        }
        status("Compiling", &self.display_module(plan, index));
        let _ = fs::remove_file(&state_path);
        self.crust.run(plan.pkg.dir(), &args, &node.name)?;
        write_file(&state_path, &format_state(&current))
    }

    /** Compiles a target's `.cr` sources. They have no interface, so they always rebuild. */
    fn compile_sources(
        &self,
        plan: &package_plan,
        target: &build_target,
        sources: &[cr_source],
        externs: &BTreeMap<String, PathBuf>,
    ) -> crepe_result<Vec<PathBuf>> {
        let dir = plan.modules_dir();
        create_dir_all(&dir)?;
        let mut objects = Vec::new();
        for (i, src) in sources.iter().enumerate() {
            let stem = src
                .file
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            // `-` cannot appear in a module name, so these never collide with module outputs.
            let base = format!("cr-{}-{}-{i}-{stem}", target.kind.table_name(), target.name);
            let check = self.mode == BuildModeCheck;
            let output = dir.join(format!("{base}.{}", if check { "ll" } else { "o" }));
            let mut args =
                if check { vec!["--emit=llvm-ir".to_string()] } else { vec!["-c".to_string()] };
            args.extend([src.file.rel.clone(), "-o".to_string(), output.display().to_string()]);
            args.extend(profile_flags(self.release));
            for inc in &target.include {
                args.push("-I".to_string());
                args.push(inc.display().to_string());
            }
            args.extend(extern_args(&src.imports, externs));
            let verb = if check { "Checking" } else { "Compiling" };
            status(verb, &format!("{} ({})", src.file.rel, target.name));
            self.crust.run(plan.pkg.dir(), &args, &src.file.rel)?;
            objects.push(output);
        }
        Ok(objects)
    }

    fn build_dependency(
        &self,
        plan: &package_plan,
        externs: &BTreeMap<String, PathBuf>,
    ) -> crepe_result<()> {
        for &index in &plan.order {
            self.compile_module(plan, index, externs)?;
        }
        let Some(lib) = plan.pkg.manifest.lib_target() else {
            return Ok(());
        };
        let sources = read_cr_sources(plan, lib)?;
        let mut objects: Vec<PathBuf> = plan
            .order
            .iter()
            .map(|&i| plan.modules_dir().join(format!("{}.o", plan.graph.modules[i].name)))
            .collect();
        objects.extend(self.compile_sources(plan, lib, &sources, externs)?);
        if self.mode == BuildModeCheck || objects.is_empty() {
            return Ok(());
        }
        let out_dir = &plan.out_dir;
        let output = lib_path(out_dir, plan.pkg.name());
        let stamp = out_dir.join("state").join(format!("lib-{}.link", plan.pkg.name()));
        self.link(&output, &stamp, &["--emit=staticlib".to_string()], &objects, plan.pkg.name())?;
        Ok(())
    }

    /** Links or archives `output`, unless it is up to date. Returns whether it ran. */
    fn link(
        &self,
        output: &Path,
        stamp: &Path,
        mode_args: &[String],
        inputs: &[PathBuf],
        what: &str,
    ) -> crepe_result<bool> {
        let mut args = mode_args.to_vec();
        args.push("-o".to_string());
        args.push(output.display().to_string());
        args.extend(inputs.iter().map(|p| p.display().to_string()));
        let print = fingerprint(&self.crust.identity, &args);
        if !link_needed(output, inputs, print, stamp) {
            return Ok(false);
        }
        if let Some(parent) = output.parent() {
            create_dir_all(parent)?;
        }
        let _ = fs::remove_file(stamp);
        // Archives are rewritten whole, so stale members never linger.
        if mode_args.iter().any(|a| a == "--emit=staticlib") {
            let _ = fs::remove_file(output);
        }
        self.crust.run(self.set.root.dir(), &args, what)?;
        write_file(stamp, &hex64(print))?;
        Ok(true)
    }

    fn finish_target(
        &self,
        plan: &package_plan,
        tp: &target_plan,
        externs: &BTreeMap<String, PathBuf>,
        dep_libs: &[PathBuf],
    ) -> crepe_result<built_target> {
        let target = &tp.target;
        let cr_objects = self.compile_sources(plan, target, &tp.sources, externs)?;
        let kind = target.kind.table_name();
        let output = match target.kind {
            TargetKindBin => self.build_dir.join(&target.name),
            TargetKindLib => lib_path(&self.build_dir, &target.name),
            TargetKindTest => self.build_dir.join("tests").join(&target.name),
        };
        if self.mode == BuildModeCheck {
            return Ok(built_target { target: target.clone(), output });
        }
        let dir = plan.modules_dir();
        let mut inputs: Vec<PathBuf> = tp
            .modules
            .iter()
            .map(|&i| dir.join(format!("{}.o", plan.graph.modules[i].name)))
            .collect();
        inputs.extend(cr_objects);
        let stamp = self.build_dir.join("state").join(format!("{kind}-{}.link", target.name));
        if target.kind == TargetKindLib {
            let header = self.build_dir.join("include").join(format!("{}.h", target.name));
            let relinked = self.link(
                &output,
                &stamp,
                &["--emit=staticlib".to_string()],
                &inputs,
                &target.name,
            )?;
            if relinked || !header.is_file() {
                self.write_header(plan, tp, &header, externs)?;
            }
        } else {
            inputs.extend(dep_libs.iter().cloned());
            self.link(&output, &stamp, &[], &inputs, &target.name)?;
        }
        Ok(built_target { target: target.clone(), output })
    }

    /**
     * Generates the C header for a library's `[[crust::c_abi]]` exports. crust resolves
     * same-package imports from the directory of `-o`, so the header is written next to
     * the interfaces and then copied to `include/`.
     */
    fn write_header(
        &self,
        plan: &package_plan,
        tp: &target_plan,
        header: &Path,
        externs: &BTreeMap<String, PathBuf>,
    ) -> crepe_result<()> {
        let Some(root) = tp.root_module else {
            return Ok(());
        };
        let node = &plan.graph.modules[root];
        let staged = plan.modules_dir().join(format!("{}.h", tp.target.name));
        let mut args = vec![
            "--emit=header".to_string(),
            "-o".to_string(),
            staged.display().to_string(),
            node.file.rel.clone(),
        ];
        args.extend(extern_args(&node.imports, externs));
        self.crust.run(plan.pkg.dir(), &args, &format!("{}.h", tp.target.name))?;
        if let Some(parent) = header.parent() {
            create_dir_all(parent)?;
        }
        fs::copy(&staged, header).map(|_| ()).map_err(|e| {
            crepe_error::new(format!(
                "could not copy `{}` to `{}`: {e}",
                staged.display(),
                header.display()
            ))
        })
    }
}

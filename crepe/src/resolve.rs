/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use crate::git;
use crate::lockfile::{load_lock, lock_file, locked_package, save_lock};
use crate::manifest::{self, DepSource::*, dependency, package_manifest};
use crate::util::{crepe_error, crepe_result, normalize, relative_path, status};

// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq)]
pub enum PackageSource {
    PackageSourceRoot,
    PackageSourcePath(PathBuf),
    PackageSourceGit { url: String, rev: Option<String>, commit: String },
}

pub use PackageSource::*;

#[derive(Clone, Debug)]
pub struct package {
    pub manifest: package_manifest,
    pub source: PackageSource,
}

impl package {
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn dir(&self) -> &Path {
        &self.manifest.root_dir
    }

    pub fn dep_names(&self) -> Vec<String> {
        self.manifest.dependencies.iter().map(|d| d.name.clone()).collect()
    }
}

/** The root package and every package it depends on, directly or not. */
#[derive(Clone, Debug)]
pub struct package_set {
    pub root: package,
    pub deps: BTreeMap<String, package>,
    /** Dependency names, each after the packages it depends on. */
    pub order: Vec<String>,
}

// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolveMode {
    /** Use locked commits where the manifest still matches the lock_file. */
    ResolveModeLocked,
    /** Re-resolve every git dependency, or only the named one. */
    ResolveModeUpdate(Option<String>),
}

pub use ResolveMode::*;

/** A dependency waiting to be resolved, with the package that asked for it. */
struct pending_dep {
    requester: String,
    requester_dir: PathBuf,
    dep: dependency,
}

/** Resolves and fetches every dependency, then writes `crepe.lock` if it changed. */
pub fn resolve(root: package_manifest, mode: &ResolveMode) -> crepe_result<package_set> {
    let old_lock = load_lock(&root.root_dir)?.unwrap_or_default();
    if let ResolveModeUpdate(Some(name)) = mode {
        if old_lock.find(name).is_none() && !root.dependencies.iter().any(|d| &d.name == name) {
            return Err(crepe_error::new(format!("package `{name}` is not a dependency")));
        }
    }
    let root_pkg = package { manifest: root, source: PackageSourceRoot };
    let mut deps: BTreeMap<String, package> = BTreeMap::new();
    let mut requested_by: BTreeMap<String, String> = BTreeMap::new();
    let mut queue: VecDeque<pending_dep> = root_pkg
        .manifest
        .dependencies
        .iter()
        .map(|d| pending_dep {
            requester: root_pkg.name().to_string(),
            requester_dir: root_pkg.dir().to_path_buf(),
            dep: d.clone(),
        })
        .collect();
    while let Some(item) = queue.pop_front() {
        let name = item.dep.name.clone();
        if name == root_pkg.name() {
            return Err(crepe_error::new(format!(
                "dependency cycle: `{}` depends on the root package `{name}`",
                item.requester
            )));
        }
        if let Some(existing) = deps.get(&name) {
            check_same_source(existing, &item, &requested_by[&name])?;
            continue;
        }
        let pkg = fetch_package(&item, &old_lock, mode)?;
        for d in &pkg.manifest.dependencies {
            queue.push_back(pending_dep {
                requester: name.clone(),
                requester_dir: pkg.dir().to_path_buf(),
                dep: d.clone(),
            });
        }
        requested_by.insert(name.clone(), item.requester.clone());
        deps.insert(name, pkg);
    }
    let order = package_order(&root_pkg, &deps)?;
    let set = package_set { root: root_pkg, deps, order };
    let new_lock = lock_for(&set);
    report_updates(&old_lock, &new_lock);
    save_lock(set.root.dir(), &new_lock)?;
    Ok(set)
}

fn path_dep_dir(requester_dir: &Path, path: &str) -> PathBuf {
    let joined = normalize(&requester_dir.join(path));
    joined.canonicalize().unwrap_or(joined)
}

fn check_same_source(
    existing: &package,
    item: &pending_dep,
    first_requester: &str,
) -> crepe_result<()> {
    let same = match (&existing.source, &item.dep.source) {
        (PackageSourcePath(dir), DepSourcePath(p)) => *dir == path_dep_dir(&item.requester_dir, p),
        (PackageSourceGit { url, rev, .. }, DepSourceGit { url: u, rev: r }) => {
            url == u && rev == r
        }
        _ => false,
    };
    if same {
        return Ok(());
    }
    Err(crepe_error::new(format!(
        "dependency `{}` is required from two different sources: {} (by `{first_requester}`) \
         and {} (by `{}`)",
        item.dep.name,
        describe_source(&existing.source),
        describe_dep(&item.dep.source),
        item.requester
    )))
}

fn describe_source(source: &PackageSource) -> String {
    match source {
        PackageSourceRoot => "the root package".to_string(),
        PackageSourcePath(dir) => format!("path `{}`", dir.display()),
        PackageSourceGit { url, rev, .. } => {
            describe_dep(&DepSourceGit { url: url.clone(), rev: rev.clone() })
        }
    }
}

fn describe_dep(source: &manifest::DepSource) -> String {
    match source {
        DepSourcePath(p) => format!("path `{p}`"),
        DepSourceGit { url, rev: Some(r) } => format!("git `{url}` at `{r}`"),
        DepSourceGit { url, rev: None } => format!("git `{url}`"),
    }
}

fn fetch_package(
    item: &pending_dep,
    old_lock: &lock_file,
    mode: &ResolveMode,
) -> crepe_result<package> {
    let name = &item.dep.name;
    let (dir, source) = match &item.dep.source {
        DepSourcePath(p) => {
            let dir = path_dep_dir(&item.requester_dir, p);
            (dir.clone(), PackageSourcePath(dir))
        }
        DepSourceGit { url, rev } => {
            let refresh = match mode {
                ResolveModeLocked => false,
                ResolveModeUpdate(None) => true,
                ResolveModeUpdate(Some(n)) => n == name,
            };
            let locked = old_lock
                .find(name)
                .filter(|l| !refresh && l.source == format!("git+{url}") && l.rev == *rev)
                .and_then(|l| l.commit.clone());
            let (dir, commit) =
                git::checkout(name, url, rev.as_deref(), locked.as_deref(), refresh)?;
            (dir, PackageSourceGit { url: url.clone(), rev: rev.clone(), commit })
        }
    };
    if !dir.join(manifest::ManifestName).is_file() {
        return Err(crepe_error::new(format!(
            "dependency `{name}` (required by `{}`): no `{}` in `{}`",
            item.requester,
            manifest::ManifestName,
            dir.display()
        )));
    }
    let m = manifest::load(&dir)?;
    if m.name != *name {
        return Err(crepe_error::new(format!(
            "dependency `{name}` (required by `{}`) points to package `{}`; the dependency key \
             must match the package name",
            item.requester, m.name
        )));
    }
    Ok(package { manifest: m, source })
}

/** Orders dependencies so each comes after the packages it depends on. Fails on a cycle. */
fn package_order(root: &package, deps: &BTreeMap<String, package>) -> crepe_result<Vec<String>> {
    let mut order = Vec::new();
    let mut state: BTreeMap<String, u8> = BTreeMap::new();
    let mut path = vec![root.name().to_string()];
    for name in root.dep_names() {
        visit_package(&name, deps, &mut state, &mut path, &mut order)?;
    }
    Ok(order)
}

fn visit_package(
    name: &str,
    deps: &BTreeMap<String, package>,
    state: &mut BTreeMap<String, u8>,
    path: &mut Vec<String>,
    order: &mut Vec<String>,
) -> crepe_result<()> {
    match state.get(name) {
        Some(2) => return Ok(()),
        Some(_) => {
            let start = path.iter().position(|p| p == name).unwrap_or(0);
            let mut cycle = path[start..].to_vec();
            cycle.push(name.to_string());
            return Err(crepe_error::new(format!("dependency cycle: {}", cycle.join(" -> "))));
        }
        None => {}
    }
    state.insert(name.to_string(), 1);
    path.push(name.to_string());
    for next in deps[name].dep_names() {
        visit_package(&next, deps, state, path, order)?;
    }
    path.pop();
    state.insert(name.to_string(), 2);
    order.push(name.to_string());
    Ok(())
}

fn lock_for(set: &package_set) -> lock_file {
    let packages = set
        .deps
        .values()
        .map(|p| {
            let (source, rev, commit) = match &p.source {
                PackageSourceGit { url, rev, commit } => {
                    (format!("git+{url}"), rev.clone(), Some(commit.clone()))
                }
                PackageSourcePath(dir) => {
                    let rel = relative_path(set.root.dir(), dir);
                    (format!("path+{}", rel.display()), None, None)
                }
                PackageSourceRoot => (String::new(), None, None),
            };
            locked_package {
                name: p.name().to_string(),
                version: p.manifest.version.clone(),
                source,
                rev,
                commit,
            }
        })
        .collect();
    lock_file { packages }
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(8)]
}

fn report_updates(old: &lock_file, new: &lock_file) {
    for p in &new.packages {
        let Some(commit) = &p.commit else { continue };
        match old.find(&p.name).and_then(|o| o.commit.as_ref()) {
            Some(prev) if prev == commit => {}
            Some(prev) => {
                status("Updating", &format!("{} {} -> {}", p.name, short(prev), short(commit)))
            }
            None => status("Locking", &format!("{} {}", p.name, short(commit))),
        }
    }
}

fn tree_label(pkg: &package) -> String {
    let source = match &pkg.source {
        PackageSourceRoot => format!("({})", pkg.dir().display()),
        PackageSourcePath(dir) => format!("({})", dir.display()),
        PackageSourceGit { url, commit, .. } => format!("({url}#{})", short(commit)),
    };
    format!("{} v{} {source}", pkg.name(), pkg.manifest.version)
}

/** Renders the dependency tree. A package seen earlier is marked `(*)` and not expanded. */
pub fn format_tree(set: &package_set) -> String {
    let mut out = format!("{}\n", tree_label(&set.root));
    let mut seen = Vec::new();
    tree_children(set, &set.root, "", &mut seen, &mut out);
    out
}

fn tree_children(
    set: &package_set,
    pkg: &package,
    prefix: &str,
    seen: &mut Vec<String>,
    out: &mut String,
) {
    let names = pkg.dep_names();
    for (i, name) in names.iter().enumerate() {
        let last = i + 1 == names.len();
        let Some(child) = set.deps.get(name) else {
            continue;
        };
        let branch = if last { "└── " } else { "├── " };
        let repeat = seen.contains(name);
        let marker = if repeat && !child.manifest.dependencies.is_empty() { " (*)" } else { "" };
        out.push_str(&format!("{prefix}{branch}{}{marker}\n", tree_label(child)));
        if !repeat {
            seen.push(name.clone());
            let next = format!("{prefix}{}", if last { "    " } else { "│   " });
            tree_children(set, child, &next, seen, out);
        }
    }
}

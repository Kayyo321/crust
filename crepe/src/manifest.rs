/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::path::{Path, PathBuf};

use crate::toml::{self, TomlValue, TomlValue::*, toml_table};
use crate::util::{crepe_error, crepe_result, is_snake_ident, read_file, warn};

pub const ManifestName: &str = "crepe.toml";

/** Names that would collide with directories crepe creates under `build/<profile>/`. */
const ReservedBinNames: [&str; 5] = ["deps", "include", "modules", "state", "tests"];

// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq)]
pub enum DepSource {
    /** A path as written in the manifest, relative to the package directory. */
    DepSourcePath(String),
    DepSourceGit {
        url: String,
        rev: Option<String>,
    },
}

pub use DepSource::*;

#[derive(Clone, Debug, PartialEq)]
pub struct dependency {
    pub name: String,
    pub source: DepSource,
}

// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetKind {
    TargetKindBin,
    TargetKindLib,
    TargetKindTest,
}

pub use TargetKind::*;

impl TargetKind {
    pub fn table_name(self) -> &'static str {
        match self {
            TargetKindBin => "bin",
            TargetKindLib => "lib",
            TargetKindTest => "test",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct build_target {
    pub kind: TargetKind,
    pub name: String,
    /** The root `.crm` module, relative to the package directory. */
    pub root: Option<PathBuf>,
    /** Old-school `.cr` sources, relative to the package directory. */
    pub sources: Vec<PathBuf>,
    /** Directories searched for `.hr` headers, relative to the package directory. */
    pub include: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct package_manifest {
    /** The absolute directory that holds `crepe.toml`. */
    pub root_dir: PathBuf,
    pub name: String,
    pub version: String,
    pub dependencies: Vec<dependency>,
    pub targets: Vec<build_target>,
}

impl package_manifest {
    pub fn lib_target(&self) -> Option<&build_target> {
        let libs: Vec<_> = self.targets.iter().filter(|t| t.kind == TargetKindLib).collect();
        libs.iter().find(|t| t.name == self.name).or(libs.first()).copied()
    }
}

/** Finds the nearest directory at or above `start` that holds `crepe.toml`. */
pub fn find_package_root(start: &Path) -> crepe_result<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(ManifestName).is_file() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    Err(crepe_error::new(format!(
        "could not find `{ManifestName}` in `{}` or any parent directory",
        start.display()
    )))
}

/** Reads and validates `<root_dir>/crepe.toml`, printing warnings. */
pub fn load(root_dir: &Path) -> crepe_result<package_manifest> {
    let path = root_dir.join(ManifestName);
    let text = read_file(&path)?;
    let exists = |rel: &str| root_dir.join(rel).is_file();
    match parse_manifest(&text, root_dir, &exists) {
        Ok((m, warnings)) => {
            for w in warnings {
                warn(&format!("{}: {w}", path.display()));
            }
            Ok(m)
        }
        Err(errors) => Err(crepe_error::many(
            errors.into_iter().map(|e| format!("{}: {e}", path.display())).collect(),
        )),
    }
}

/**
 * Validates a manifest. `exists` reports whether a package-relative file exists; it
 * decides the default targets. Returns the manifest and its warnings, or every error.
 */
pub fn parse_manifest(
    text: &str,
    root_dir: &Path,
    exists: &dyn Fn(&str) -> bool,
) -> Result<(package_manifest, Vec<String>), Vec<String>> {
    let doc = toml::parse(text).map_err(|e| vec![e])?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for key in doc.keys() {
        if !["package", "dependencies", "bin", "lib", "test"].contains(&key) {
            warnings.push(format!("unused manifest key `{key}`"));
        }
    }
    let (name, version) = parse_package(&doc, &mut errors, &mut warnings);
    let dependencies = parse_dependencies(&doc, &name, &mut errors, &mut warnings);
    let mut targets = Vec::new();
    for kind in [TargetKindBin, TargetKindLib, TargetKindTest] {
        parse_targets(&doc, kind, &mut targets, &mut errors, &mut warnings);
    }
    add_default_targets(&name, exists, &mut targets);
    check_duplicate_targets(&targets, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    let m =
        package_manifest { root_dir: root_dir.to_path_buf(), name, version, dependencies, targets };
    Ok((m, warnings))
}

pub fn name_hint(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if c == '-' || c == ' ' || c == '.' { '_' } else { c.to_ascii_lowercase() })
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
        .collect();
    out = out.trim_start_matches('_').to_string();
    if out.is_empty() || !out.starts_with(|c: char| c.is_ascii_lowercase()) {
        out.insert_str(0, "pkg_");
    }
    out
}

pub fn invalid_name_message(what: &str, name: &str) -> String {
    format!(
        "invalid {what} name `{name}`: names must be snake_case identifiers ([a-z][a-z0-9_]*)\n\
         help: try `{}`",
        name_hint(name)
    )
}

fn parse_package(
    doc: &toml_table,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> (String, String) {
    let Some(TomlValueTable(pkg)) = doc.get("package") else {
        errors.push("missing `[package]` table".to_string());
        return (String::new(), String::new());
    };
    for key in pkg.keys() {
        if !["name", "version", "description", "license", "authors"].contains(&key) {
            warnings.push(format!("unused manifest key `package.{key}`"));
        }
    }
    let name = string_field(pkg, "name", "package", errors).unwrap_or_default();
    if !name.is_empty() && !is_snake_ident(&name) {
        errors.push(invalid_name_message("package", &name));
    }
    let version = string_field(pkg, "version", "package", errors).unwrap_or_default();
    if !version.is_empty() && !is_semver(&version) {
        errors
            .push(format!("invalid version `{version}`: expected MAJOR.MINOR.PATCH, like `0.1.0`"));
    }
    (name, version)
}

fn is_semver(text: &str) -> bool {
    let core = text.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<_> = core.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn string_field(
    table: &toml_table,
    key: &str,
    ctx: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    match table.get(key) {
        Some(TomlValueString(s)) => Some(s.clone()),
        Some(other) => {
            errors.push(format!("`{ctx}.{key}` must be a string, found {}", other.type_name()));
            None
        }
        None => {
            errors.push(format!("missing `{ctx}.{key}`"));
            None
        }
    }
}

fn optional_string(
    table: &toml_table,
    key: &str,
    ctx: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    table.get(key)?;
    string_field(table, key, ctx, errors)
}

fn parse_dependencies(
    doc: &toml_table,
    package: &str,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Vec<dependency> {
    let deps = match doc.get("dependencies") {
        None => return Vec::new(),
        Some(TomlValueTable(t)) => t,
        Some(other) => {
            errors.push(format!("`dependencies` must be a table, found {}", other.type_name()));
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for (name, value) in &deps.entries {
        if !is_snake_ident(name) {
            errors.push(invalid_name_message("dependency", name));
            continue;
        }
        if name == package {
            errors.push(format!("package `{package}` cannot depend on itself"));
            continue;
        }
        if let Some(source) = parse_dep_source(name, value, errors, warnings) {
            out.push(dependency { name: name.clone(), source });
        }
    }
    out
}

fn parse_dep_source(
    name: &str,
    value: &TomlValue,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<DepSource> {
    let ctx = format!("dependencies.{name}");
    let table = match value {
        TomlValueString(v) => {
            errors.push(format!(
                "dependency `{name} = \"{v}\"`: registry dependencies are not supported in 0.1.0\n\
                 help: use `{name} = {{ path = \"...\" }}` or \
                 `{name} = {{ git = \"...\", rev = \"...\" }}`"
            ));
            return None;
        }
        TomlValueTable(t) => t,
        other => {
            errors.push(format!("`{ctx}` must be a table, found {}", other.type_name()));
            return None;
        }
    };
    for key in table.keys() {
        if !["path", "git", "rev", "version"].contains(&key) {
            warnings.push(format!("unused manifest key `{ctx}.{key}`"));
        }
    }
    let path = optional_string(table, "path", &ctx, errors);
    let git = optional_string(table, "git", &ctx, errors);
    let rev = optional_string(table, "rev", &ctx, errors);
    match (path, git) {
        (Some(_), Some(_)) => {
            errors.push(format!("dependency `{name}` has both `path` and `git`; use one"));
            None
        }
        (Some(p), None) => {
            if rev.is_some() {
                errors.push(format!("dependency `{name}`: `rev` is only allowed with `git`"));
            }
            Some(DepSourcePath(p))
        }
        (None, Some(url)) => Some(DepSourceGit { url, rev }),
        (None, None) => {
            let has_version = table.get("version").is_some();
            let why =
                if has_version { "registry dependencies are not supported in 0.1.0; " } else { "" };
            errors.push(format!("dependency `{name}`: {why}expected `path` or `git`"));
            None
        }
    }
}

fn is_target_name(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with(['-', '.'])
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn path_list(table: &toml_table, key: &str, ctx: &str, errors: &mut Vec<String>) -> Vec<PathBuf> {
    match table.get(key) {
        None => Vec::new(),
        Some(TomlValueArray(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    TomlValueString(s) => out.push(PathBuf::from(s)),
                    other => errors.push(format!(
                        "`{ctx}.{key}` must hold strings, found {}",
                        other.type_name()
                    )),
                }
            }
            out
        }
        Some(other) => {
            errors.push(format!(
                "`{ctx}.{key}` must be an array of strings, found {}",
                other.type_name()
            ));
            Vec::new()
        }
    }
}

fn parse_targets(
    doc: &toml_table,
    kind: TargetKind,
    out: &mut Vec<build_target>,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let key = kind.table_name();
    let tables = match doc.get(key) {
        None => return,
        Some(TomlValueArray(items)) => items,
        Some(_) => {
            errors.push(format!("`{key}` must be an array of tables: write `[[{key}]]`"));
            return;
        }
    };
    for item in tables {
        let TomlValueTable(table) = item else {
            errors.push(format!("`{key}` must be an array of tables: write `[[{key}]]`"));
            continue;
        };
        if let Some(target) = parse_target(table, kind, errors, warnings) {
            out.push(target);
        }
    }
}

fn parse_target(
    table: &toml_table,
    kind: TargetKind,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<build_target> {
    let ctx = format!("[[{}]]", kind.table_name());
    for key in table.keys() {
        if !["name", "root", "sources", "include"].contains(&key) {
            warnings.push(format!("unused manifest key `{key}` in {ctx}"));
        }
    }
    let before = errors.len();
    let name = string_field(table, "name", &ctx, errors)?;
    let ctx = format!("{ctx} `{name}`");
    if !is_target_name(&name) {
        errors.push(format!("{ctx}: target names may contain only letters, digits, `_`, and `-`"));
    }
    if kind == TargetKindBin && ReservedBinNames.contains(&name.as_str()) {
        errors.push(format!("{ctx}: `{name}` is reserved for a directory under `build/`"));
    }
    let root = optional_string(table, "root", &ctx, errors).map(PathBuf::from);
    let sources = path_list(table, "sources", &ctx, errors);
    let include = path_list(table, "include", &ctx, errors);
    let has_sources = table.get("sources").is_some();
    match (&root, has_sources) {
        (Some(_), true) => errors.push(format!("{ctx}: set `root` or `sources`, not both")),
        (None, false) => {
            errors.push(format!("{ctx}: set `root` (a .crm module) or `sources` (.cr files)"))
        }
        (None, true) if sources.is_empty() => errors.push(format!("{ctx}: `sources` is empty")),
        _ => {}
    }
    if root.is_some() && !include.is_empty() {
        errors.push(format!("{ctx}: `include` applies only to `sources`"));
    }
    let all_paths = root.iter().chain(&sources).chain(&include);
    for path in all_paths {
        if path.is_absolute() {
            errors.push(format!("{ctx}: `{}` must be relative to the package", path.display()));
        }
    }
    if let Some(r) = &root {
        if r.extension().is_none_or(|e| e != "crm") {
            errors.push(format!("{ctx}: `root` must be a `.crm` module, found `{}`", r.display()));
        }
    }
    for s in &sources {
        if s.extension().is_none_or(|e| e != "cr") {
            errors.push(format!("{ctx}: `sources` must be `.cr` files, found `{}`", s.display()));
        }
    }
    if errors.len() > before {
        return None;
    }
    Some(build_target { kind, name, root, sources, include })
}

/** Cargo-style defaults: a kind with no tables of its own gets its conventional root. */
fn add_default_targets(name: &str, exists: &dyn Fn(&str) -> bool, targets: &mut Vec<build_target>) {
    if name.is_empty() {
        return;
    }
    let defaults = [(TargetKindBin, "src/main.crm"), (TargetKindLib, "src/lib.crm")];
    for (kind, root) in defaults {
        if !targets.iter().any(|t| t.kind == kind) && exists(root) {
            targets.push(build_target {
                kind,
                name: name.to_string(),
                root: Some(PathBuf::from(root)),
                sources: Vec::new(),
                include: Vec::new(),
            });
        }
    }
    targets.sort_by_key(|t| t.kind);
}

fn check_duplicate_targets(targets: &[build_target], errors: &mut Vec<String>) {
    for (i, a) in targets.iter().enumerate() {
        if targets[..i].iter().any(|b| b.kind == a.kind && b.name == a.name) {
            errors.push(format!("two [[{}]] targets are named `{}`", a.kind.table_name(), a.name));
        }
    }
    let libs = targets.iter().filter(|t| t.kind == TargetKindLib).count();
    if libs > 1 {
        errors.push("a package can have at most one [[lib]] target".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_with(
        text: &str,
        files: &[&str],
    ) -> Result<(package_manifest, Vec<String>), Vec<String>> {
        let exists = |rel: &str| files.contains(&rel);
        parse_manifest(text, Path::new("/pkg"), &exists)
    }

    const Package: &str = "[package]\nname = \"app\"\nversion = \"0.1.0\"\n";

    #[test]
    fn reads_package_and_dependencies() {
        let text = format!(
            "{Package}[dependencies]\nutil = {{ path = \"../util\" }}\n\
             parse = {{ git = \"https://example.invalid/p\", rev = \"a1b2\" }}\n"
        );
        let (m, warnings) = parse_with(&text, &[]).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(m.name, "app");
        assert_eq!(m.dependencies[0].source, DepSourcePath("../util".to_string()));
        let git = DepSourceGit {
            url: "https://example.invalid/p".to_string(),
            rev: Some("a1b2".to_string()),
        };
        assert_eq!(m.dependencies[1].source, git);
    }

    #[test]
    fn rejects_registry_dependencies() {
        let text = format!("{Package}[dependencies]\nmathx = \"1.2\"\n");
        let errors = parse_with(&text, &[]).unwrap_err();
        assert!(
            errors[0].contains("registry dependencies are not supported in 0.1.0"),
            "{errors:?}"
        );
    }

    #[test]
    fn rejects_bad_package_names_with_hint() {
        let errors =
            parse_with("[package]\nname = \"My-App\"\nversion = \"0.1.0\"\n", &[]).unwrap_err();
        assert!(errors[0].contains("snake_case"));
        assert!(errors[0].contains("try `my_app`"), "{errors:?}");
    }

    #[test]
    fn rejects_missing_fields_and_bad_versions() {
        let errors = parse_with("[package]\nname = \"app\"\nversion = \"1.0\"\n", &[]).unwrap_err();
        assert!(errors[0].contains("invalid version"));
        let errors = parse_with("[dependencies]\n", &[]).unwrap_err();
        assert!(errors[0].contains("missing `[package]`"));
    }

    #[test]
    fn adds_default_targets_per_kind() {
        let (m, _) = parse_with(Package, &["src/main.crm", "src/lib.crm"]).unwrap();
        let kinds: Vec<_> = m.targets.iter().map(|t| (t.kind, t.name.as_str())).collect();
        assert_eq!(kinds, vec![(TargetKindBin, "app"), (TargetKindLib, "app")]);

        let text = format!("{Package}[[bin]]\nname = \"tool\"\nroot = \"src/tool.crm\"\n");
        let (m, _) = parse_with(&text, &["src/main.crm"]).unwrap();
        assert_eq!(m.targets.len(), 1);
        assert_eq!(m.targets[0].name, "tool");
    }

    #[test]
    fn reads_source_targets() {
        let text = format!(
            "{Package}[[bin]]\nname = \"legacy-tool\"\nsources = [\"src/tool/main.cr\"]\n\
             include = [\"src/tool\"]\n[[test]]\nname = \"t\"\nroot = \"tests/t.crm\"\n"
        );
        let (m, _) = parse_with(&text, &[]).unwrap();
        assert_eq!(m.targets[0].sources, vec![PathBuf::from("src/tool/main.cr")]);
        assert_eq!(m.targets[0].include, vec![PathBuf::from("src/tool")]);
        assert_eq!(m.targets[1].kind, TargetKindTest);
    }

    #[test]
    fn rejects_invalid_targets() {
        let both =
            format!("{Package}[[bin]]\nname = \"a\"\nroot = \"src/a.crm\"\nsources = [\"a.cr\"]\n");
        assert!(parse_with(&both, &[]).unwrap_err()[0].contains("not both"));
        let neither = format!("{Package}[[bin]]\nname = \"a\"\n");
        assert!(parse_with(&neither, &[]).unwrap_err()[0].contains("set `root`"));
        let dup = format!(
            "{Package}[[bin]]\nname = \"a\"\nroot = \"src/a.crm\"\n\
             [[bin]]\nname = \"a\"\nroot = \"src/b.crm\"\n"
        );
        assert!(parse_with(&dup, &[]).unwrap_err()[0].contains("two [[bin]] targets"));
        let reserved = format!("{Package}[[bin]]\nname = \"modules\"\nroot = \"src/a.crm\"\n");
        assert!(parse_with(&reserved, &[]).unwrap_err()[0].contains("reserved"));
        let wrong_ext = format!("{Package}[[bin]]\nname = \"a\"\nroot = \"src/a.cr\"\n");
        assert!(parse_with(&wrong_ext, &[]).unwrap_err()[0].contains(".crm"));
    }

    #[test]
    fn rejects_bad_dependency_tables() {
        let both = format!("{Package}[dependencies]\nx = {{ path = \"a\", git = \"b\" }}\n");
        assert!(parse_with(&both, &[]).unwrap_err()[0].contains("both"));
        let rev = format!("{Package}[dependencies]\nx = {{ path = \"a\", rev = \"b\" }}\n");
        assert!(parse_with(&rev, &[]).unwrap_err()[0].contains("`rev`"));
        let selfdep = format!("{Package}[dependencies]\napp = {{ path = \"a\" }}\n");
        assert!(parse_with(&selfdep, &[]).unwrap_err()[0].contains("itself"));
    }

    #[test]
    fn hints_snake_case_names() {
        assert_eq!(name_hint("Hello World"), "hello_world");
        assert_eq!(name_hint("9lives"), "pkg_9lives");
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::path::Path;

use crate::manifest::{self, ManifestName, invalid_name_message};
use crate::toml::quote;
use crate::util::{
    absolute, crepe_error, crepe_result, fail, is_snake_ident, read_file, relative_path, status,
    write_file,
};

/** The `#` form of the file header (CODE_STYLE.md, section 1). */
const HashHeader: &str = "# Ko, (Baczynski-Bruce) 2026; Apache 2.0";

/**
 * C reserved words and Crust keywords. A package name becomes a module name, so it
 * cannot be one of these.
 */
const ReservedNames: [&str; 53] = [
    "alignas",
    "alignof",
    "auto",
    "bool",
    "break",
    "case",
    "char",
    "const",
    "constexpr",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "false",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "nullptr",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "struct",
    "switch",
    "thread_local",
    "true",
    "typedef",
    "typeof",
    "typeof_unqual",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "let",
    "mut",
    "unsafe",
    "import",
    "export",
    "module",
    "from",
    "as",
];

pub fn manifest_template(name: &str) -> String {
    format!("{HashHeader}\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[dependencies]\n")
}

pub fn main_template(name: &str) -> String {
    format!(
        "/**\n* Ko, (Baczynski-Bruce) 2026; Apache 2.0\n*/\nmodule {name};\n\n\
         #include <stdio.h>\n\n\
         int main(void) {{\n    unsafe {{\n        \
         // SAFETY: the format string is a literal and matches the arguments.\n        \
         printf(\"Hello from {name}!\\n\");\n    }}\n    return 0;\n}}\n"
    )
}

pub fn gitignore_template() -> String {
    format!("{HashHeader}\n\nbuild/\n")
}

fn check_package_name(name: &str) -> crepe_result<()> {
    if !is_snake_ident(name) {
        return fail(invalid_name_message("package", name));
    }
    if ReservedNames.contains(&name) {
        return fail(format!(
            "invalid package name `{name}`: it is a reserved word\nhelp: try `{name}_pkg`"
        ));
    }
    Ok(())
}

/** Writes a new package into `dir`, keeping any `src/` roots that already exist. */
fn create_package(dir: &Path, name: &str) -> crepe_result<()> {
    write_file(&dir.join(ManifestName), &manifest_template(name))?;
    let src = dir.join("src");
    if !src.join("main.crm").exists() && !src.join("lib.crm").exists() {
        write_file(&src.join("main.crm"), &main_template(name))?;
    }
    let gitignore = dir.join(".gitignore");
    if gitignore.exists() {
        let text = read_file(&gitignore)?;
        if !text.lines().any(|l| matches!(l.trim(), "build/" | "/build/" | "build" | "/build")) {
            let sep = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
            write_file(&gitignore, &format!("{text}{sep}build/\n"))?;
        }
    } else {
        write_file(&gitignore, &gitignore_template())?;
    }
    status("Created", &format!("package `{name}`"));
    Ok(())
}

/** `crepe new <name>` */
pub fn new_package(name: &str) -> crepe_result<()> {
    check_package_name(name)?;
    let dir = absolute(Path::new(name))?;
    if dir.exists() {
        return fail(format!("destination `{}` already exists", dir.display()));
    }
    create_package(&dir, name)
}

/** `crepe init` */
pub fn init_package() -> crepe_result<()> {
    let dir = absolute(Path::new("."))?;
    if dir.join(ManifestName).exists() {
        return fail(format!("`{ManifestName}` already exists in `{}`", dir.display()));
    }
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    check_package_name(&name).map_err(|mut e| {
        e.messages[0].push_str(
            "\nhelp: the package is named after the directory; rename it, or use \
             `crepe new <name>`",
        );
        e
    })?;
    create_package(&dir, &name)
}

/** Where `crepe add` takes a dependency from. */
// CODE_STYLE.md, section 2: enumerators are prefixed with their enum's name.
#[derive(Clone, Debug, PartialEq)]
pub enum AddSource {
    AddSourcePath(String),
    AddSourceGit { url: String, rev: Option<String> },
    AddSourceRegistry(Option<String>),
}

pub use AddSource::*;

/** `crepe add <name> ...` */
pub fn add_dependency(root: &Path, name: &str, source: AddSource) -> crepe_result<()> {
    if !is_snake_ident(name) {
        return fail(invalid_name_message("dependency", name));
    }
    let value = match source {
        AddSourceRegistry(version) => {
            let what = version.map(|v| format!("{name}@{v}")).unwrap_or_else(|| name.to_string());
            return fail(format!(
                "cannot add `{what}`: registry dependencies are not supported in 0.1\n\
                 help: use `crepe add {name} --path <dir>` or `crepe add {name} --git <url>`"
            ));
        }
        AddSourcePath(dir) => {
            let abs = absolute(Path::new(&dir))?;
            let dep = manifest::load(&abs).map_err(|_| {
                crepe_error::new(format!(
                    "`{}` is not a crepe package (no readable `{ManifestName}`)",
                    abs.display()
                ))
            })?;
            if dep.name != name {
                return fail(format!(
                    "the package at `{}` is named `{}`, not `{name}`",
                    abs.display(),
                    dep.name
                ));
            }
            let rel = relative_path(root, &abs);
            format!("{{ path = {} }}", quote(&rel.to_string_lossy()))
        }
        AddSourceGit { url, rev: Some(rev) } => {
            format!("{{ git = {}, rev = {} }}", quote(&url), quote(&rev))
        }
        AddSourceGit { url, rev: None } => format!("{{ git = {} }}", quote(&url)),
    };
    let path = root.join(ManifestName);
    let text = read_file(&path)?;
    let (without, replaced) = match remove_dependency_text(&text, name) {
        Some(t) => (t, true),
        None => (text, false),
    };
    let updated = add_dependency_text(&without, name, &value);
    manifest::parse_manifest(&updated, root, &|_| false).map_err(crepe_error::many)?;
    write_file(&path, &updated)?;
    status(if replaced { "Updated" } else { "Adding" }, &format!("{name} = {value}"));
    Ok(())
}

/** `crepe remove <name>` */
pub fn remove_dependency(root: &Path, name: &str) -> crepe_result<()> {
    let path = root.join(ManifestName);
    let text = read_file(&path)?;
    let Some(updated) = remove_dependency_text(&text, name) else {
        return fail(format!("`{name}` is not a dependency of this package"));
    };
    write_file(&path, &updated)?;
    status("Removing", name);
    Ok(())
}

/** The table name of a `[header]` line, if the line is one. `[[x]]` returns `[x]`. */
fn header_name(line: &str) -> Option<String> {
    let t = line.trim();
    let t = t.split(" #").next().unwrap_or(t).trim();
    if t.starts_with("[[") && t.ends_with("]]") {
        return Some(format!("[{}]", t[2..t.len() - 2].trim()));
    }
    if t.starts_with('[') && t.ends_with(']') {
        return Some(
            t[1..t.len() - 1]
                .split('.')
                .map(|p| p.trim().trim_matches('"'))
                .collect::<Vec<_>>()
                .join("."),
        );
    }
    None
}

fn line_key(line: &str) -> Option<String> {
    let (key, _) = line.split_once('=')?;
    Some(key.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
}

/**
 * Removes `name` from `[dependencies]`, or its `[dependencies.name]` table. Returns
 * `None` when the manifest does not list it.
 */
pub fn remove_dependency_text(text: &str, name: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut section = String::new();
    let mut found = false;
    let sub_table = format!("dependencies.{name}");
    for line in text.lines() {
        if let Some(h) = header_name(line) {
            section = h;
            if section == sub_table {
                found = true;
                continue;
            }
        } else if section == sub_table {
            continue;
        } else if section == "dependencies" && line_key(line).as_deref() == Some(name) {
            found = true;
            continue;
        }
        out.push(line);
    }
    if !found {
        return None;
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    Some(joined)
}

/** Adds `name = value` at the end of `[dependencies]`, creating the table if needed. */
pub fn add_dependency_text(text: &str, name: &str, value: &str) -> String {
    let entry = format!("{name} = {value}");
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|l| header_name(l).as_deref() == Some("dependencies"))
    else {
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(&format!("[dependencies]\n{entry}\n"));
        return out;
    };
    let end = lines[start + 1..]
        .iter()
        .position(|l| header_name(l).is_some())
        .map_or(lines.len(), |i| start + 1 + i);
    // Insert after the last entry, before the blank lines that separate the next table.
    let mut at = end;
    while at > start + 1 && lines[at - 1].trim().is_empty() {
        at -= 1;
    }
    let mut out: Vec<&str> = lines[..at].to_vec();
    out.push(&entry);
    out.extend(&lines[at..]);
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensure::ensure;

    #[test]
    fn templates_start_with_the_header() {
        ensure(manifest_template("app"))
            .named("manifest template")
            .starts_with("# Ko, (Baczynski-Bruce) 2026; Apache 2.0\n");
        ensure(gitignore_template()).named(".gitignore template").ends_with("build/\n");
        let main = main_template("app");
        let expected = [
            "/**",
            "* Ko, (Baczynski-Bruce) 2026; Apache 2.0",
            "*/",
            "module app;",
            "",
            "#include <stdio.h>",
            "",
            "int main(void) {",
            "    unsafe {",
            "        // SAFETY: the format string is a literal and matches the arguments.",
            "        printf(\"Hello from app!\\n\");",
            "    }",
            "    return 0;",
            "}",
            "",
        ]
        .join("\n");
        ensure(main).named("main template").is(expected);
    }

    #[test]
    fn manifest_template_is_valid() {
        let parsed =
            manifest::parse_manifest(&manifest_template("app"), Path::new("/x"), &|_| true);
        let (m, warnings) = ensure(parsed).named("template parse").is_ok().into_value();
        ensure(m.name.as_str()).named("package name").is("app");
        ensure(warnings).named("warnings").is_empty();
    }

    #[test]
    fn rejects_reserved_and_invalid_names() {
        ensure(check_package_name("int")).is_err();
        ensure(check_package_name("My-App")).is_err();
        ensure(check_package_name("my_app")).is_ok();
    }

    #[test]
    fn adds_dependencies_to_existing_table() {
        let text = "[package]\nname = \"a\"\n\n[dependencies]\nx = { path = \"../x\" }\n\n\
                    [[bin]]\nname = \"a\"\n";
        let out = add_dependency_text(text, "y", "{ path = \"../y\" }");
        ensure(out).is("[package]\nname = \"a\"\n\n[dependencies]\nx = { path = \"../x\" }\n\
             y = { path = \"../y\" }\n\n[[bin]]\nname = \"a\"\n");
    }

    #[test]
    fn adds_dependencies_table_when_missing() {
        let out = add_dependency_text("[package]\nname = \"a\"\n", "y", "{ git = \"u\" }");
        ensure(out).is("[package]\nname = \"a\"\n\n[dependencies]\ny = { git = \"u\" }\n");
        let empty = add_dependency_text("[dependencies]\n", "y", "{ git = \"u\" }");
        ensure(empty).is("[dependencies]\ny = { git = \"u\" }\n");
    }

    #[test]
    fn removes_dependencies_in_both_forms() {
        let text = "[dependencies]\nx = { path = \"../x\" }\ny = { path = \"../y\" }\n";
        ensure(remove_dependency_text(text, "x"))
            .is_some()
            .is("[dependencies]\ny = { path = \"../y\" }\n");
        ensure(remove_dependency_text(text, "z")).is_none();
        let table = "[dependencies.x]\npath = \"../x\"\n[[bin]]\nname = \"a\"\n";
        ensure(remove_dependency_text(table, "x")).is_some().is("[[bin]]\nname = \"a\"\n");
    }
}

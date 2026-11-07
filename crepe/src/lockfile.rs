/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::path::Path;

use crate::toml::{self, TomlValue::*, quote, toml_table};
use crate::util::{crepe_error, crepe_result, read_file, write_if_changed};

pub const LockName: &str = "crepe.lock";
pub const LockVersion: i64 = 1;

/** One resolved dependency. `source` is `path+<dir>` or `git+<url>`. */
#[derive(Clone, Debug, PartialEq)]
pub struct locked_package {
    pub name: String,
    pub version: String,
    pub source: String,
    /** The `rev` the manifest asked for, so a changed `rev` re-resolves. */
    pub rev: Option<String>,
    /** The exact commit for a git dependency. */
    pub commit: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct lock_file {
    pub packages: Vec<locked_package>,
}

impl lock_file {
    pub fn find(&self, name: &str) -> Option<&locked_package> {
        self.packages.iter().find(|p| p.name == name)
    }
}

/**
 * Writes the lockfile. The format is TOML with no comments, so it has no file header
 * (CODE_STYLE.md, section 1 exempts `crepe.lock`).
 */
pub fn format_lock(lock: &lock_file) -> String {
    let mut packages = lock.packages.clone();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = format!("version = {LockVersion}\n");
    for p in &packages {
        out.push_str("\n[[package]]\n");
        out.push_str(&format!("name = {}\n", quote(&p.name)));
        out.push_str(&format!("version = {}\n", quote(&p.version)));
        out.push_str(&format!("source = {}\n", quote(&p.source)));
        if let Some(rev) = &p.rev {
            out.push_str(&format!("rev = {}\n", quote(rev)));
        }
        if let Some(commit) = &p.commit {
            out.push_str(&format!("commit = {}\n", quote(commit)));
        }
    }
    out
}

fn field(table: &toml_table, key: &str) -> Result<Option<String>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(TomlValueString(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("`{key}` must be a string")),
    }
}

pub fn parse_lock(text: &str) -> Result<lock_file, String> {
    let doc = toml::parse(text)?;
    match doc.get("version") {
        Some(TomlValueInteger(LockVersion)) => {}
        Some(TomlValueInteger(v)) => return Err(format!("unsupported lockfile version {v}")),
        _ => return Err("missing `version`".to_string()),
    }
    let mut lock = lock_file::default();
    let items = match doc.get("package") {
        None => return Ok(lock),
        Some(TomlValueArray(items)) => items,
        Some(_) => return Err("`package` must be an array of tables".to_string()),
    };
    for item in items {
        let TomlValueTable(t) = item else {
            return Err("`package` must be an array of tables".to_string());
        };
        let name = field(t, "name")?.ok_or("a package is missing `name`")?;
        let source =
            field(t, "source")?.ok_or_else(|| format!("package `{name}` is missing `source`"))?;
        lock.packages.push(locked_package {
            version: field(t, "version")?.unwrap_or_default(),
            rev: field(t, "rev")?,
            commit: field(t, "commit")?,
            name,
            source,
        });
    }
    Ok(lock)
}

/** Reads `crepe.lock` next to the manifest, if there is one. */
pub fn load_lock(package_dir: &Path) -> crepe_result<Option<lock_file>> {
    let path = package_dir.join(LockName);
    if !path.is_file() {
        return Ok(None);
    }
    let text = read_file(&path)?;
    parse_lock(&text).map(Some).map_err(|e| {
        crepe_error::new(format!("{}: {e}\nhelp: delete it and run `crepe update`", path.display()))
    })
}

pub fn save_lock(package_dir: &Path, lock: &lock_file) -> crepe_result<()> {
    write_if_changed(&package_dir.join(LockName), &format_lock(lock))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensure::ensure;

    #[test]
    fn lockfile_round_trips_sorted() {
        let lock = lock_file {
            packages: vec![
                locked_package {
                    name: "zeta".into(),
                    version: "0.2.0".into(),
                    source: "git+https://example.invalid/zeta".into(),
                    rev: Some("main".into()),
                    commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
                },
                locked_package {
                    name: "alpha".into(),
                    version: "0.1.0".into(),
                    source: "path+../alpha".into(),
                    rev: None,
                    commit: None,
                },
            ],
        };
        let text = format_lock(&lock);
        ensure(&text)
            .named("lockfile text")
            .starts_with("version = 1\n\n[[package]]\nname = \"alpha\"")
            .cannot_contain('#');
        let parsed = ensure(parse_lock(&text)).is_ok().into_value();
        ensure(parsed.packages.len()).named("packages after a round trip").is(2);
        ensure(parsed.find("zeta")).is(lock.packages.first());
    }

    #[test]
    fn rejects_unknown_versions() {
        ensure(parse_lock("version = 2\n")).is_err().contains("unsupported");
        ensure(parse_lock("")).is_err();
        ensure(parse_lock("version = 1\n")).is_ok().is(lock_file::default());
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/** Exit code for an ordinary failure: a bad manifest, a compile error, a failed test. */
pub const ExitFailure: i32 = 1;
/** Exit code for a malformed command line. */
pub const ExitUsage: i32 = 2;

/** An error that ends the command. Each message is printed as `error: <message>`. */
#[derive(Debug)]
pub struct crepe_error {
    pub messages: Vec<String>,
    pub exit_code: i32,
}

pub type crepe_result<T> = Result<T, crepe_error>;

impl crepe_error {
    pub fn new(message: impl Into<String>) -> crepe_error {
        crepe_error { messages: vec![message.into()], exit_code: ExitFailure }
    }

    pub fn usage(message: impl Into<String>) -> crepe_error {
        crepe_error { messages: vec![message.into()], exit_code: ExitUsage }
    }

    pub fn many(messages: Vec<String>) -> crepe_error {
        crepe_error { messages, exit_code: ExitFailure }
    }
}

pub fn fail<T>(message: impl Into<String>) -> crepe_result<T> {
    Err(crepe_error::new(message))
}

pub fn io_error(action: &str, path: &Path, err: &io::Error) -> crepe_error {
    crepe_error::new(format!("could not {action} `{}`: {err}", path.display()))
}

pub fn read_file(path: &Path) -> crepe_result<String> {
    fs::read_to_string(path).map_err(|e| io_error("read", path, &e))
}

pub fn create_dir_all(path: &Path) -> crepe_result<()> {
    fs::create_dir_all(path).map_err(|e| io_error("create directory", path, &e))
}

/** Writes `contents`, creating parent directories as needed. */
pub fn write_file(path: &Path, contents: &str) -> crepe_result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    fs::write(path, contents).map_err(|e| io_error("write", path, &e))
}

/** Writes `contents` only if the file does not already hold exactly that text. */
pub fn write_if_changed(path: &Path, contents: &str) -> crepe_result<bool> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == contents {
            return Ok(false);
        }
    }
    write_file(path, contents)?;
    Ok(true)
}

/** Prints a cargo-style status line: the verb right-aligned in 12 columns. */
pub fn status(verb: &str, message: &str) {
    eprintln!("{verb:>12} {message}");
}

pub fn warn(message: &str) {
    eprintln!("warning: {message}");
}

/** 64-bit FNV-1a. Used for change detection, not for security. */
#[derive(Clone, Copy)]
pub struct fnv_hasher {
    state: u64,
}

impl fnv_hasher {
    pub fn new() -> fnv_hasher {
        fnv_hasher { state: 0xcbf2_9ce4_8422_2325 }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /** Hashes a field and a terminator, so `["ab", "c"]` and `["a", "bc"]` differ. */
    pub fn field(&mut self, text: &str) {
        self.update(text.as_bytes());
        self.update(&[0]);
    }

    pub fn finish(&self) -> u64 {
        self.state
    }
}

pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = fnv_hasher::new();
    hasher.update(bytes);
    hasher.finish()
}

pub fn hex64(value: u64) -> String {
    format!("{value:016x}")
}

/** `[a-z][a-z0-9_]*`: package names, dependency names, and module name segments. */
pub fn is_snake_ident(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/** A dotted module name whose segments are snake_case: `net.http`. */
pub fn is_module_name(text: &str) -> bool {
    !text.is_empty() && text.split('.').all(is_snake_ident)
}

/** Removes `.` and folds `..` without touching the file system. */
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = matches!(out.components().next_back(), Some(Component::Normal(_)));
                if can_pop {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/** The path of `target` relative to the directory `base`. Both should be absolute. */
pub fn relative_path(base: &Path, target: &Path) -> PathBuf {
    let base = normalize(base);
    let target = normalize(target);
    let base_parts: Vec<_> = base.components().collect();
    let target_parts: Vec<_> = target.components().collect();
    let common = base_parts.iter().zip(&target_parts).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return target;
    }
    let mut out = PathBuf::new();
    for _ in common..base_parts.len() {
        out.push("..");
    }
    for part in &target_parts[common..] {
        out.push(part.as_os_str());
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/** A path for messages: relative to `root` when it is inside `root`, else as given. */
pub fn display_path(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
        _ => path.display().to_string(),
    }
}

/** Makes `path` absolute against the current directory, without resolving links. */
pub fn absolute(path: &Path) -> crepe_result<PathBuf> {
    if path.is_absolute() {
        return Ok(normalize(path));
    }
    let cwd = std::env::current_dir()
        .map_err(|e| crepe_error::new(format!("could not read the current directory: {e}")))?;
    Ok(normalize(&cwd.join(path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensure::ensure;

    #[test]
    fn accepts_snake_case_names_only() {
        for name in ["app", "my_app2"] {
            ensure(is_snake_ident(name)).named(format!("`{name}` is snake_case")).is_true();
        }
        for name in ["MyApp", "my-app", "2app", "_app", ""] {
            ensure(is_snake_ident(name)).named(format!("`{name}` is snake_case")).is_false();
        }
    }

    #[test]
    fn accepts_dotted_module_names() {
        ensure(is_module_name("net.http")).named("`net.http` is a module name").is_true();
        for name in ["net..http", "net.Http", ""] {
            ensure(is_module_name(name)).named(format!("`{name}` is a module name")).is_false();
        }
    }

    #[test]
    fn computes_relative_paths() {
        let rel = relative_path(Path::new("/a/b/c"), Path::new("/a/d"));
        ensure(rel).is(PathBuf::from("../../d"));
        let same = relative_path(Path::new("/a/b"), Path::new("/a/b"));
        ensure(same).is(PathBuf::from("."));
        let inside = relative_path(Path::new("/a"), Path::new("/a/b/./c"));
        ensure(inside).is(PathBuf::from("b/c"));
    }

    #[test]
    fn normalizes_parent_components() {
        ensure(normalize(Path::new("/a/b/../c/./d"))).is(PathBuf::from("/a/c/d"));
        ensure(normalize(Path::new("../x"))).is(PathBuf::from("../x"));
    }

    #[test]
    fn hash_fields_are_delimited() {
        let mut a = fnv_hasher::new();
        a.field("ab");
        a.field("c");
        let mut b = fnv_hasher::new();
        b.field("a");
        b.field("bc");
        ensure(a.finish()).named("hash of fields `ab`, `c`").cannot_be(b.finish());
    }
}

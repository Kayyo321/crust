/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Reads the test's own source, so a failure can show the line that failed.
use std::cell::Cell;
use std::path::{Path, PathBuf};

thread_local! {
    static Hidden: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` with source lines left out of every report made on this thread. A test of a failure
/// message uses it so the message cannot match its own expected text, which the source line
/// would otherwise echo back.
pub fn without_source<R>(f: impl FnOnce() -> R) -> R {
    struct restore(bool);
    impl Drop for restore {
        fn drop(&mut self) {
            Hidden.with(|h| h.set(self.0));
        }
    }
    let _restore = restore(Hidden.with(|h| h.replace(true)));
    f()
}

/// Where the compiler says a file is, found from where the test runs. `cargo test` runs from
/// the package folder while `file!()` is relative to the workspace, so look in each parent.
fn locate(file: &str) -> Option<PathBuf> {
    let path = Path::new(file);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_path_buf());
    }
    let cwd = std::env::current_dir().ok()?;
    cwd.ancestors().map(|dir| dir.join(path)).find(|candidate| candidate.is_file())
}

/// The lines of `file` from `line - before` to `line + after`, with their 1-based numbers.
/// `None` when the file cannot be read or has no such line.
pub fn snippet(file: &str, line: u32, before: u32, after: u32) -> Option<Vec<(u32, String)>> {
    if Hidden.with(|h| h.get()) {
        return None;
    }
    let text = std::fs::read_to_string(locate(file)?).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    if line == 0 || line as usize > lines.len() {
        return None;
    }
    let first = line.saturating_sub(before).max(1);
    let last = (line + after).min(lines.len() as u32);
    Some((first..=last).map(|n| (n, lines[n as usize - 1].to_string())).collect())
}

/// How many characters of `line` to underline for the call that starts at `column` (1-based):
/// the method name, and its argument list if it closes on this line.
pub fn call_width(line: &str, column: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let start = column.saturating_sub(1);
    if start >= chars.len() {
        return 1;
    }
    let mut i = start;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    if i >= chars.len() || chars[i] != '(' {
        return (i - start).max(1);
    }
    let mut depth = 0;
    let mut in_string = false;
    let mut j = i;
    while j < chars.len() {
        let c = chars[j];
        if in_string {
            if c == '\\' {
                j += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == '\'' && chars.get(j + 2) == Some(&'\'') {
            j += 2;
        } else if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return j + 1 - start;
            }
        }
        j += 1;
    }
    // The arguments continue on the next line: underline to the end of this one.
    (chars.len() - start).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure;

    #[test]
    fn call_width_covers_the_name_and_its_arguments() {
        ensure(call_width("    ensure(x).is(2);", 15)).is(5);
        ensure(call_width("    ensure(x).contains(\"a(\");", 15)).is(14);
        ensure(call_width("ensure(x).is_some();", 11)).is(9);
    }

    #[test]
    fn call_width_without_parens_is_the_name() {
        ensure(call_width("let f = ensure;", 9)).is(6);
    }

    #[test]
    fn call_width_of_an_open_call_runs_to_the_end_of_the_line() {
        ensure(call_width("x.is(", 3)).is(3);
    }

    #[test]
    fn call_width_is_at_least_one() {
        ensure(call_width("", 1)).is(1);
        ensure(call_width("ab", 9)).is(1);
        ensure(call_width("..", 1)).is(1);
    }

    #[test]
    fn call_width_skips_character_literals() {
        ensure(call_width("x.is('(');", 3)).is(7);
    }

    #[test]
    fn a_missing_file_has_no_snippet() {
        ensure(snippet("no/such/source.rs", 1, 2, 1).is_none()).is_true();
    }

    #[test]
    fn a_line_past_the_end_has_no_snippet() {
        ensure(snippet(file!(), 1_000_000, 2, 1).is_none()).is_true();
        ensure(snippet(file!(), 0, 2, 1).is_none()).is_true();
    }

    #[test]
    fn this_file_can_be_found_from_the_package_folder() {
        let lines = snippet(file!(), 2, 1, 1).unwrap();
        ensure(lines.iter().map(|(n, _)| *n).collect::<Vec<_>>()).is(vec![1, 2, 3]);
        ensure(lines[1].1.as_str()).is("* Ko, (Baczynski-Bruce) 2026; Apache 2.0");
    }

    #[test]
    fn without_source_hides_snippets_and_restores_them() {
        ensure(without_source(|| snippet(file!(), 2, 1, 1)).is_none()).is_true();
        ensure(snippet(file!(), 2, 1, 1).is_some()).is_true();
        // Nested scopes put back the outer state, not the default.
        without_source(|| {
            without_source(|| ());
            ensure(snippet(file!(), 2, 1, 1).is_none()).is_true();
        });
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Small text helpers that make failures readable: undoing Debug escapes so text can be shown
// as text, shortening type names, and explaining how two values differ.
use crate::diff;
use crate::rendering::report;
use std::fmt::Debug;

/// The text inside a Debug-printed string (`"a\\nb"`) or char (`'x'`), with the escapes undone.
/// `None` when `debug` is anything else, such as a list of strings or a struct.
pub fn debug_text(debug: &str) -> Option<String> {
    let quote = debug.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = debug.strip_prefix(quote)?.strip_suffix(quote)?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == quote {
            // A bare quote inside means this is not one literal, like `"a" "b"`.
            return None;
        }
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            'u' => {
                if chars.next()? != '{' {
                    return None;
                }
                let mut hex = String::new();
                loop {
                    match chars.next()? {
                        '}' => break,
                        h => hex.push(h),
                    }
                }
                out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// A type name without its module paths: `alloc::vec::Vec<i32>` becomes `Vec<i32>`.
pub fn short_type(name: &str) -> String {
    let mut out = String::new();
    let mut segment = String::new();
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            segment.push(c);
        } else if c == ':' && chars.peek() == Some(&':') {
            chars.next();
            segment.clear();
        } else {
            out.push_str(&segment);
            segment.clear();
            out.push(c);
        }
    }
    out.push_str(&segment);
    out
}

/// `text` without ANSI color sequences.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        if chars.next() == Some('[') {
            for end in chars.by_ref() {
                if end.is_ascii_alphabetic() {
                    break;
                }
            }
        }
    }
    out
}

/// `text` cut to `max` characters, with `...` where it was cut.
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(3)).collect();
    format!("{kept}...")
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("{n} {one}") } else { format!("{n} {one}s") }
}

/// Adds the "found" row for `value`. Text is shown as text and long values as a block, so
/// nothing is a wall of escapes. `block` is false when another block already shows the value.
pub fn found<V: Debug + ?Sized>(r: report, value: &V, block: bool) -> report {
    let short = format!("{value:?}");
    if let Some(text) = debug_text(&short) {
        if short.starts_with('"') && (text.contains('\n') || text.chars().count() > 80) {
            let lines = text.lines().count().max(1);
            let r = r.row(
                "found",
                format!("text of {}, {}", plural(lines, "line"), plural(text.len(), "byte")),
            );
            return if block { r.text_block("found text", &text) } else { r };
        }
        return r.row("found", short);
    }
    if short.chars().count() > 100 {
        let r = r.row("found", clip(&short, 100));
        return if block { r.text_block("found (pretty)", &format!("{value:#?}")) } else { r };
    }
    r.row("found", short)
}

/// Adds how two texts differ: a line diff when either has several lines, a pointer to the first
/// differing character otherwise. Adds nothing when they are the same.
pub fn text_diff(r: report, wanted: &str, got: &str) -> report {
    if wanted == got {
        r
    } else if wanted.contains('\n') || got.contains('\n') {
        text_difference(r, wanted, got)
    } else {
        one_line_difference(r, wanted, got)
    }
}

/// Adds "found" and, when the two values are text or multi-line, how they differ.
pub fn difference<W: Debug + ?Sized, F: Debug + ?Sized>(r: report, wanted: &W, got: &F) -> report {
    let wanted_debug = format!("{wanted:?}");
    let got_debug = format!("{got:?}");
    if let (Some(w), Some(f)) = (debug_text(&wanted_debug), debug_text(&got_debug)) {
        if w.contains('\n') || f.contains('\n') {
            return text_difference(found(r, got, false), &w, &f);
        }
        return one_line_difference(found(r, got, false), &w, &f);
    }
    let wanted_pretty = format!("{wanted:#?}");
    let got_pretty = format!("{got:#?}");
    if wanted_pretty.contains('\n') || got_pretty.contains('\n') {
        let r = found(r, got, false);
        let w: Vec<&str> = wanted_pretty.lines().collect();
        let f: Vec<&str> = got_pretty.lines().collect();
        return match diff::render(&w, &f) {
            Some(lines) => r.diff_block("difference (- wanted, + found)", lines),
            None => r.text_block("found (pretty)", &got_pretty),
        };
    }
    found(r, got, true)
}

fn text_difference(r: report, wanted: &str, got: &str) -> report {
    let w: Vec<&str> = wanted.lines().collect();
    let f: Vec<&str> = got.lines().collect();
    if w == f {
        return r.help("the lines are the same; only a trailing newline or line ending differs");
    }
    let r = match diff::render(&w, &f) {
        Some(lines) => r.diff_block("difference (- wanted, + found)", lines),
        None => r.text_block("found text", got),
    };
    similarity_hint(r, wanted, got)
}

fn one_line_difference(r: report, wanted: &str, got: &str) -> report {
    let at = wanted.chars().zip(got.chars()).take_while(|(a, b)| a == b).count();
    let w = wanted.chars().nth(at);
    let f = got.chars().nth(at);
    let what = match (w, f) {
        (Some(w), Some(f)) => format!("at character {}: wanted {w:?}, found {f:?}", at + 1),
        (Some(w), None) => format!("found ends at character {}: wanted {w:?} next", at + 1),
        (None, Some(f)) => format!("wanted ends at character {}: found {f:?} next", at + 1),
        (None, None) => return r,
    };
    similarity_hint(r.row("differs", what), wanted, got)
}

/// A help line when the two texts are the same apart from whitespace at the edges or letter case.
fn similarity_hint(r: report, wanted: &str, got: &str) -> report {
    if wanted.trim() == got.trim() {
        r.help("the values differ only in surrounding whitespace")
    } else if wanted.to_lowercase() == got.to_lowercase() {
        r.help("the values differ only in letter case")
    } else {
        r
    }
}

/// A help line when `needle` is in `haystack` once letter case is ignored.
pub fn case_hint(r: report, haystack_debug: &str, needle_debug: &str) -> report {
    match (debug_text(haystack_debug), debug_text(needle_debug)) {
        (Some(h), Some(n)) if !h.contains(&n) && h.to_lowercase().contains(&n.to_lowercase()) => {
            r.help("it is there when letter case is ignored")
        }
        _ => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure;

    #[test]
    fn debug_text_undoes_string_escapes() {
        ensure(debug_text(r#""a\nb\t\"q\" \\ \u{e9}""#)).is_some().is("a\nb\t\"q\" \\ é");
        ensure(debug_text("'x'")).is_some().is("x");
        ensure(debug_text(r"'\n'")).is_some().is("\n");
    }

    #[test]
    fn debug_text_refuses_what_is_not_one_literal() {
        ensure(debug_text("[\"a\", \"b\"]")).is_none();
        ensure(debug_text("\"a\" \"b\"")).is_none();
        ensure(debug_text("Some(\"a\")")).is_none();
        ensure(debug_text("42")).is_none();
        ensure(debug_text("\"bad \\q escape\"")).is_none();
        ensure(debug_text("\"")).is_none();
    }

    #[test]
    fn short_type_drops_module_paths() {
        ensure(short_type("alloc::vec::Vec<i32>")).is("Vec<i32>");
        ensure(short_type("core::option::Option<alloc::string::String>")).is("Option<String>");
        ensure(short_type("&str")).is("&str");
        ensure(short_type("(i32, alloc::string::String)")).is("(i32, String)");
        ensure(short_type("[u8; 4]")).is("[u8; 4]");
    }

    #[test]
    fn strip_ansi_removes_color_sequences_only() {
        ensure(strip_ansi("\x1b[1;31mred\x1b[0m and plain")).is("red and plain");
        ensure(strip_ansi("no escapes")).is("no escapes");
    }

    #[test]
    fn clip_marks_where_it_cut() {
        ensure(clip("short", 10)).is("short");
        ensure(clip("abcdefghij", 8)).is("abcde...");
        ensure(clip("abcdefghij", 10)).is("abcdefghij");
    }
}

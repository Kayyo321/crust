/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Terminal colors for failure reports. Colors are off unless stderr is a terminal, so logs
// and captured test output stay plain. `ENSURE_COLOR=always` or `never` overrides the guess,
// `NO_COLOR` turns colors off, and `CLICOLOR_FORCE` turns them on.
use std::io::IsTerminal;

/// What a piece of a report is, which decides its color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint {
    PaintError,
    PaintGutter,
    PaintCaret,
    PaintAdded,
    PaintRemoved,
    PaintHelp,
    PaintBold,
    PaintDim,
}

/// Whether reports should carry colors right now.
pub fn enabled() -> bool {
    decide(
        std::env::var("ENSURE_COLOR").ok().as_deref(),
        std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
        std::env::var("CLICOLOR_FORCE").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
        std::io::stderr().is_terminal(),
    )
}

/// The decision, with the environment passed in so it can be tested.
pub fn decide(
    ensure_color: Option<&str>,
    no_color: bool,
    clicolor_force: Option<&str>,
    term: Option<&str>,
    stderr_is_terminal: bool,
) -> bool {
    match ensure_color {
        Some("always") => return true,
        Some("never") => return false,
        _ => {}
    }
    if no_color {
        return false;
    }
    if clicolor_force.is_some_and(|v| !v.is_empty() && v != "0") {
        return true;
    }
    term != Some("dumb") && stderr_is_terminal
}

/// `text` in the color of `paint`, or unchanged when `on` is false.
pub fn paint(paint: Paint, text: &str, on: bool) -> String {
    if !on || text.is_empty() {
        return text.to_string();
    }
    let code = match paint {
        Paint::PaintError => "1;31",
        Paint::PaintGutter => "1;34",
        Paint::PaintCaret => "1;31",
        Paint::PaintAdded => "32",
        Paint::PaintRemoved => "31",
        Paint::PaintHelp => "1;36",
        Paint::PaintBold => "1",
        Paint::PaintDim => "2",
    };
    format!("\x1b[{code}m{text}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure;

    #[test]
    fn ensure_color_overrides_everything() {
        ensure(decide(Some("always"), true, None, Some("dumb"), false)).is_true();
        ensure(decide(Some("never"), false, Some("1"), None, true)).is_false();
    }

    #[test]
    fn no_color_beats_a_terminal() {
        ensure(decide(None, true, None, Some("xterm"), true)).is_false();
    }

    #[test]
    fn clicolor_force_turns_color_on_unless_it_is_zero_or_empty() {
        ensure(decide(None, false, Some("1"), None, false)).is_true();
        ensure(decide(None, false, Some("0"), None, false)).is_false();
        ensure(decide(None, false, Some(""), None, false)).is_false();
    }

    #[test]
    fn a_terminal_gets_color_unless_it_is_dumb() {
        ensure(decide(None, false, None, Some("xterm"), true)).is_true();
        ensure(decide(None, false, None, Some("dumb"), true)).is_false();
        ensure(decide(None, false, None, None, false)).is_false();
    }

    #[test]
    fn paint_leaves_text_alone_when_off_or_empty() {
        ensure(paint(Paint::PaintError, "x", false)).is("x");
        ensure(paint(Paint::PaintError, "", true)).is("");
        ensure(paint(Paint::PaintError, "x", true)).is("\x1b[1;31mx\x1b[0m");
    }
}

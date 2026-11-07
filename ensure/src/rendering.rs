/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The failure report, in the shape of a Rust compiler diagnostic:
//
//     error: ensure failed: exit code
//      --> tests/abi.rs:41:48
//       |
//     41 |     ensure(code).named("exit code").is(0);
//       |                                      ^^^^^ wanted exactly 0, found 3
//       |
//       = wanted: exactly 0
//       =  found: 3
//       =   type: i32
//
// The test runners build the same reports for their own failures, so every failure in this
// repo reads alike.
use crate::paint::{self, Paint};
use crate::pretty;
use crate::source;
use std::panic::Location;
use std::process::Output;

/// How a line inside a block is colored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    ToneNone,
    ToneAdded,
    ToneRemoved,
    ToneDim,
}

/// Block lines past this many are folded into a count.
const MaxBlockLines: usize = 40;

/// Lines of source shown above and below the failing line.
const ContextBefore: u32 = 2;
const ContextAfter: u32 = 1;

struct place {
    file: String,
    line: u32,
    /// 1-based, in characters. 0 means unknown, which shows no underline.
    column: u32,
    /// Characters to underline, when the caller knows better than the call scanner.
    width: Option<usize>,
}

struct block {
    title: String,
    lines: Vec<(Tone, String)>,
}

/// A failure report. Build it with the methods below, then `render` it.
pub struct report {
    title: String,
    place: Option<place>,
    label: Option<String>,
    rows: Vec<(String, String)>,
    blocks: Vec<block>,
    helps: Vec<String>,
}

impl report {
    /// A report headed `error: <title>`.
    pub fn error(title: impl Into<String>) -> report {
        report {
            title: title.into(),
            place: None,
            label: None,
            rows: Vec::new(),
            blocks: Vec::new(),
            helps: Vec::new(),
        }
    }

    /// Where it happened. `column` is 1-based in characters, or 0 when unknown.
    pub fn at(mut self, file: impl Into<String>, line: u32, column: u32) -> report {
        self.place = Some(place { file: file.into(), line, column, width: None });
        self
    }

    /// Where it happened, from a `#[track_caller]` location.
    pub fn at_location(self, location: &Location<'_>) -> report {
        self.at(location.file(), location.line(), location.column())
    }

    /// How many characters to underline, instead of the call that starts at the column.
    pub fn underline(mut self, width: usize) -> report {
        if let Some(place) = &mut self.place {
            place.width = Some(width);
        }
        self
    }

    /// The text after the underline. Without one, a "wanted ..., found ..." label is made from
    /// the rows of those names, when they are short.
    pub fn label(mut self, text: impl Into<String>) -> report {
        self.label = Some(text.into());
        self
    }

    /// A `name: value` line below the snippet. Names are right-aligned.
    pub fn row(mut self, name: impl Into<String>, value: impl Into<String>) -> report {
        self.rows.push((name.into(), value.into()));
        self
    }

    /// A titled block of plain lines.
    pub fn text_block(self, title: impl Into<String>, text: &str) -> report {
        let lines = text.lines().map(|l| (Tone::ToneNone, l.to_string())).collect();
        self.diff_block(title, lines)
    }

    /// A titled block of lines with a color each, like a diff.
    pub fn diff_block(mut self, title: impl Into<String>, lines: Vec<(Tone, String)>) -> report {
        self.blocks.push(block { title: title.into(), lines });
        self
    }

    /// Shows how `found` differs from `wanted` when both are text: a line diff, or the first
    /// character where they part.
    pub fn text_diff(self, wanted: &str, found: &str) -> report {
        pretty::text_diff(self, wanted, found)
    }

    /// Shows a finished process: a `found` row with its exit status, then a block for each of
    /// stdout and stderr that has output.
    pub fn process(self, output: &Output) -> report {
        let status = match output.status.code() {
            Some(code) => format!("exit status {code}"),
            // No code means a signal on Unix; the status text names it.
            None => output.status.to_string(),
        };
        let mut r = self.row("found", status);
        for (title, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
            let text = String::from_utf8_lossy(bytes);
            r = if text.trim().is_empty() {
                r.row(title, "(empty)")
            } else {
                r.text_block(title, text.trim_end_matches('\n'))
            };
        }
        r
    }

    /// A `help:` line at the end.
    pub fn help(mut self, text: impl Into<String>) -> report {
        self.helps.push(text.into());
        self
    }

    fn row_value(&self, name: &str) -> Option<&str> {
        self.rows.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    /// The label under the underline.
    fn label_text(&self) -> String {
        if let Some(label) = &self.label {
            return label.clone();
        }
        let short = |s: &str| !s.contains('\n') && s.chars().count() <= 40;
        match (self.row_value("wanted"), self.row_value("found")) {
            (Some(w), Some(f)) if short(w) && short(f) => format!("wanted {w}, found {f}"),
            (Some(w), _) if short(w) => format!("wanted {w}"),
            (Some(_), _) => "see below".to_string(),
            _ => String::new(),
        }
    }

    /// The report as text, in color when the terminal wants it.
    pub fn render(&self) -> String {
        self.render_with(paint::enabled())
    }

    /// The report as text, with or without ANSI colors.
    pub fn render_with(&self, color: bool) -> String {
        let p = |kind: Paint, text: &str| paint::paint(kind, text, color);
        let snippet = self
            .place
            .as_ref()
            .and_then(|pl| source::snippet(&pl.file, pl.line, ContextBefore, ContextAfter));
        let numbers = snippet
            .as_ref()
            .and_then(|s| s.last().map(|(n, _)| *n))
            .or(self.place.as_ref().map(|pl| pl.line))
            .unwrap_or(1);
        let width = numbers.to_string().len();
        let pad = " ".repeat(width);
        let bar = p(Paint::PaintGutter, "|");
        let eq = p(Paint::PaintGutter, "=");
        let mut out =
            format!("{}: {}\n", p(Paint::PaintError, "error"), p(Paint::PaintBold, &self.title));

        if let Some(pl) = &self.place {
            let at = if pl.column > 0 {
                format!("{}:{}:{}", pl.file, pl.line, pl.column)
            } else {
                format!("{}:{}", pl.file, pl.line)
            };
            out.push_str(&format!("{pad}{} {at}\n", p(Paint::PaintGutter, "-->")));
        }
        if let (Some(pl), Some(lines)) = (&self.place, &snippet) {
            out.push_str(&format!("{pad} {bar}\n"));
            for (n, text) in lines {
                let shown = text.replace('\t', "    ");
                let number = p(Paint::PaintGutter, &format!("{n:>width$}"));
                out.push_str(&format!("{number} {bar} {shown}\n"));
                if *n == pl.line && pl.column > 0 {
                    let before: usize = text
                        .chars()
                        .take(pl.column as usize - 1)
                        .map(|c| if c == '\t' { 4 } else { 1 })
                        .sum();
                    let carets =
                        pl.width.unwrap_or_else(|| source::call_width(text, pl.column as usize));
                    let label = self.label_text();
                    let label = if label.is_empty() { label } else { format!(" {label}") };
                    out.push_str(&format!(
                        "{pad} {bar} {}{}{}\n",
                        " ".repeat(before),
                        p(Paint::PaintCaret, &"^".repeat(carets.max(1))),
                        p(Paint::PaintCaret, &label)
                    ));
                }
            }
        }

        let has_detail = !self.rows.is_empty() || !self.blocks.is_empty() || !self.helps.is_empty();
        if snippet.is_some() && has_detail {
            out.push_str(&format!("{pad} {bar}\n"));
        }
        let name_width = self.rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
        for (name, value) in &self.rows {
            out.push_str(&format!("{pad} {eq} {name:>name_width$}: {value}\n"));
        }
        for block in &self.blocks {
            out.push_str(&format!("{pad} {bar}\n{pad} {eq} {}:\n", block.title));
            for (tone, line) in block.lines.iter().take(MaxBlockLines) {
                let kind = match tone {
                    Tone::ToneNone => None,
                    Tone::ToneAdded => Some(Paint::PaintAdded),
                    Tone::ToneRemoved => Some(Paint::PaintRemoved),
                    Tone::ToneDim => Some(Paint::PaintDim),
                };
                let line = match kind {
                    Some(kind) => p(kind, line),
                    None => line.clone(),
                };
                out.push_str(&format!("{pad} {bar}   {line}\n"));
            }
            if block.lines.len() > MaxBlockLines {
                let more = block.lines.len() - MaxBlockLines;
                out.push_str(&format!(
                    "{pad} {bar}   {}\n",
                    p(Paint::PaintDim, &format!("... {more} more lines"))
                ));
            }
        }
        for help in &self.helps {
            out.push_str(&format!("{pad} {eq} {}: {help}\n", p(Paint::PaintHelp, "help")));
        }
        out.truncate(out.trim_end().len());
        out
    }
}

/// A report text without color, for code that has the rendered text and wants it plain.
pub fn plain(text: &str) -> String {
    pretty::strip_ansi(text)
}

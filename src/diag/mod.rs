/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
pub mod codes;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// What kind of file a source is. The kind picks the language mode
/// (docs/LANGUAGE.md, section 2).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum FileKind {
    FileKindC,
    FileKindH,
    FileKindCr,
    FileKindHr,
    FileKindCrm,
    FileKindOther,
}
pub use FileKind::*;

impl FileKind {
    pub fn from_path(p: &Path) -> FileKind {
        match p.extension().and_then(|e| e.to_str()) {
            Some("c") => FileKindC,
            Some("h") => FileKindH,
            Some("cr") => FileKindCr,
            Some("hr") => FileKindHr,
            Some("crm") => FileKindCrm,
            _ => FileKindOther,
        }
    }

    /// Crust files reserve the Crust keywords and allow Crust syntax.
    pub fn is_crust(self) -> bool {
        matches!(self, FileKindCr | FileKindHr | FileKindCrm)
    }
}

/// A byte range in one source file.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Default)]
pub struct span {
    pub file: u32,
    pub lo: u32,
    pub hi: u32,
}

impl span {
    pub fn to(self, other: span) -> span {
        if self.file != other.file || other.hi < self.lo {
            return self;
        }
        span { file: self.file, lo: self.lo, hi: other.hi.max(self.hi) }
    }

    pub fn is_dummy(self) -> bool {
        self.file == u32::MAX
    }

    pub fn dummy() -> span {
        span { file: u32::MAX, lo: 0, hi: 0 }
    }
}

pub struct source_file {
    pub path: PathBuf,
    pub display: String,
    pub text: Rc<str>,
    pub line_starts: Vec<u32>,
    pub kind: FileKind,
}

thread_local! {
    static Files: RefCell<Vec<Rc<source_file>>> = const { RefCell::new(Vec::new()) };
    static Sink: RefCell<Vec<diagnostic>> = const { RefCell::new(Vec::new()) };
}

pub fn add_file(path: PathBuf, display: String, text: Rc<str>, kind: FileKind) -> u32 {
    let mut line_starts = vec![0u32];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i as u32 + 1);
        }
    }
    let f = source_file { path, display, text, line_starts, kind };
    Files.with(|fs| {
        let mut fs = fs.borrow_mut();
        fs.push(Rc::new(f));
        fs.len() as u32 - 1
    })
}

pub fn file(id: u32) -> Rc<source_file> {
    Files.with(|fs| fs.borrow()[id as usize].clone())
}

pub fn file_kind(id: u32) -> FileKind {
    if id == u32::MAX {
        return FileKindOther;
    }
    Files.with(|fs| fs.borrow()[id as usize].kind)
}

/// Line and column, both 1-based, of a byte offset.
pub fn line_col(file_id: u32, offset: u32) -> (u32, u32) {
    let f = file(file_id);
    let line = match f.line_starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i - 1,
    };
    let col = offset - f.line_starts[line] + 1;
    (line as u32 + 1, col)
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Severity {
    SeverityError,
    SeverityWarning,
    SeverityNote,
}
pub use Severity::*;

#[derive(Clone, Debug)]
pub struct diagnostic {
    pub severity: Severity,
    pub code: Option<&'static str>,
    pub message: String,
    pub span: Option<span>,
    pub label: Option<String>,
    pub secondary: Vec<(span, String)>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

/// Builder returned by [`error`] and [`warning`]. The diagnostic is recorded
/// when the builder is dropped.
pub struct diag_builder {
    d: Option<diagnostic>,
}

impl diag_builder {
    pub fn label(mut self, text: impl Into<String>) -> Self {
        if let Some(d) = &mut self.d {
            d.label = Some(text.into());
        }
        self
    }

    pub fn secondary(mut self, sp: span, text: impl Into<String>) -> Self {
        if let Some(d) = &mut self.d {
            if !sp.is_dummy() {
                d.secondary.push((sp, text.into()));
            }
        }
        self
    }

    pub fn note(mut self, text: impl Into<String>) -> Self {
        if let Some(d) = &mut self.d {
            d.notes.push(text.into());
        }
        self
    }

    pub fn help(mut self, text: impl Into<String>) -> Self {
        if let Some(d) = &mut self.d {
            d.help = Some(text.into());
        }
        self
    }
}

impl Drop for diag_builder {
    fn drop(&mut self) {
        if let Some(d) = self.d.take() {
            Sink.with(|s| s.borrow_mut().push(d));
        }
    }
}

fn make(severity: Severity, code: &'static str, sp: span, msg: String) -> diag_builder {
    debug_assert!(code.is_empty() || codes::lookup(code).is_some(), "unregistered code {code}");
    diag_builder {
        d: Some(diagnostic {
            severity,
            code: if code.is_empty() { None } else { Some(code) },
            message: msg,
            span: if sp.is_dummy() { None } else { Some(sp) },
            label: None,
            secondary: Vec::new(),
            notes: Vec::new(),
            help: None,
        }),
    }
}

pub fn error(code: &'static str, sp: span, msg: impl Into<String>) -> diag_builder {
    make(SeverityError, code, sp, msg.into())
}

pub fn warning(code: &'static str, sp: span, msg: impl Into<String>) -> diag_builder {
    make(SeverityWarning, code, sp, msg.into())
}

pub fn error_count() -> usize {
    Sink.with(|s| s.borrow().iter().filter(|d| d.severity == SeverityError).count())
}

pub fn take_all() -> Vec<diagnostic> {
    Sink.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

/// Drops every diagnostic recorded after `mark`. Used when a speculative
/// parse is abandoned.
pub fn mark() -> usize {
    Sink.with(|s| s.borrow().len())
}

pub fn truncate(mark: usize) {
    Sink.with(|s| s.borrow_mut().truncate(mark));
}

fn location(sp: span) -> String {
    let f = file(sp.file);
    let (l, c) = line_col(sp.file, sp.lo);
    format!("{}:{}:{}", f.display, l, c)
}

fn render_snippet(out: &mut String, sp: span, label: &str, primary: bool) {
    let f = file(sp.file);
    let (line, col) = line_col(sp.file, sp.lo);
    let start = f.line_starts[line as usize - 1] as usize;
    let end = f.text[start..].find('\n').map(|e| start + e).unwrap_or(f.text.len());
    let text = &f.text[start..end];
    let width = line.to_string().len();
    let pad = " ".repeat(width);
    let hi = (sp.hi as usize).min(end).max(sp.lo as usize + 1);
    let len = hi - sp.lo as usize;
    let marker = if primary { "^" } else { "-" };
    out.push_str(&format!("{pad} |\n{line} | {}\n", text.replace('\t', " ")));
    let lead: String = " ".repeat(col as usize - 1);
    out.push_str(&format!("{pad} | {lead}{}", marker.repeat(len.max(1))));
    if !label.is_empty() {
        out.push(' ');
        out.push_str(label);
    }
    out.push('\n');
}

pub fn render_human(d: &diagnostic) -> String {
    let sev = match d.severity {
        SeverityError => "error",
        SeverityWarning => "warning",
        SeverityNote => "note",
    };
    let mut out = match d.code {
        Some(c) => format!("{sev}[{c}]: {}\n", d.message),
        None => format!("{sev}: {}\n", d.message),
    };
    if let Some(sp) = d.span {
        out.push_str(&format!("  --> {}\n", location(sp)));
        render_snippet(&mut out, sp, d.label.as_deref().unwrap_or(""), true);
    }
    for (sp, text) in &d.secondary {
        out.push_str(&format!("  ::: {}\n", location(*sp)));
        render_snippet(&mut out, *sp, text, false);
    }
    for n in &d.notes {
        out.push_str(&format!("  = note: {n}\n"));
    }
    if let Some(h) = &d.help {
        out.push_str(&format!("  = help: {h}\n"));
    }
    out
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn json_span(sp: span) -> String {
    let f = file(sp.file);
    let (l, c) = line_col(sp.file, sp.lo);
    format!(
        "{{\"file\":{},\"line\":{},\"column\":{},\"length\":{}}}",
        json_str(&f.display),
        l,
        c,
        sp.hi.saturating_sub(sp.lo)
    )
}

pub fn render_json(d: &diagnostic) -> String {
    let sev = match d.severity {
        SeverityError => "error",
        SeverityWarning => "warning",
        SeverityNote => "note",
    };
    let mut o = format!("{{\"severity\":\"{sev}\"");
    o.push_str(&format!(",\"code\":{}", d.code.map(json_str).unwrap_or("null".into())));
    o.push_str(&format!(",\"message\":{}", json_str(&d.message)));
    o.push_str(&format!(",\"span\":{}", d.span.map(json_span).unwrap_or("null".into())));
    let secs: Vec<String> = d
        .secondary
        .iter()
        .map(|(s, t)| format!("{{\"span\":{},\"label\":{}}}", json_span(*s), json_str(t)))
        .collect();
    o.push_str(&format!(",\"related\":[{}]", secs.join(",")));
    let notes: Vec<String> = d.notes.iter().map(|n| json_str(n)).collect();
    o.push_str(&format!(",\"notes\":[{}]", notes.join(",")));
    o.push_str(&format!(
        ",\"help\":{}}}",
        d.help.as_deref().map(json_str).unwrap_or("null".into())
    ));
    o
}

/// Internal compiler error: a bug in Crust, not in the user's code.
/// Panics; the driver catches the panic, reports it, and exits with code 70.
pub fn ice(msg: impl Into<String>) -> ! {
    panic!("internal compiler error: {}", msg.into())
}

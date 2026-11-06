/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
mod builtin_headers;
mod cond_expr;

use crate::diag::{self, FileKind, FileKindCrm, FileKindOther, span};
use crate::lex::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct pp_options {
    pub quote_dirs: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    pub system_dirs: Vec<PathBuf>,
    pub defines: Vec<(String, String)>,
    pub undefs: Vec<String>,
}

/// Default system include directories for the host (x86-64 Linux).
pub fn default_system_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for d in ["/usr/local/include", "/usr/include/x86_64-linux-gnu", "/usr/include"] {
        let p = PathBuf::from(d);
        if p.is_dir() {
            v.push(p);
        }
    }
    v
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum SearchDir {
    SearchDirPath(PathBuf),
    SearchDirBuiltin,
}
use SearchDir::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuiltinMacro {
    BuiltinMacroFile,
    BuiltinMacroLine,
    BuiltinMacroCounter,
    BuiltinMacroDate,
    BuiltinMacroTime,
}
use BuiltinMacro::*;

pub struct macro_def {
    name: sym,
    params: Option<Vec<sym>>,
    variadic: bool,
    body: Vec<token>,
    builtin: Option<BuiltinMacro>,
}

struct include_frame {
    file_id: u32,
    /// Index in the search list where this file was found, for `#include_next`.
    dir_index: Option<usize>,
    cond_depth: usize,
}

struct cond_frame {
    /// Tokens in this group are kept.
    active: bool,
    /// Some group of this conditional was already taken.
    taken: bool,
    seen_else: bool,
    /// The enclosing group was active.
    parent_active: bool,
    span: span,
}

pub struct preprocessor {
    opts: pp_options,
    macros: HashMap<sym, Rc<macro_def>>,
    /// Pending input, reversed: `pop` yields the next token.
    stack: Vec<token>,
    includes: Vec<include_frame>,
    conds: Vec<cond_frame>,
    once: HashSet<PathBuf>,
    lexed: HashMap<PathBuf, (u32, Rc<Vec<token>>)>,
    counter: u32,
    out: Vec<token>,
}

/// A token that marks the end of an argument list being expanded alone.
fn sentinel() -> token {
    token { kind: TokenKindEof, text: SymHashHash, span: span::dummy(), flags: 0, hideset: None }
}

fn is_sentinel(t: &token) -> bool {
    t.kind == TokenKindEof && t.text == SymHashHash
}

fn hs_contains(h: &hideset, s: sym) -> bool {
    h.as_ref().is_some_and(|v| v.contains(&s))
}

fn hs_union(h: &hideset, s: &[sym]) -> hideset {
    let mut v: Vec<sym> = h.as_ref().map(|v| (**v).clone()).unwrap_or_default();
    for x in s {
        if !v.contains(x) {
            v.push(*x);
        }
    }
    Some(Rc::new(v))
}

fn hs_intersect(a: &hideset, b: &hideset) -> Vec<sym> {
    match (a, b) {
        (Some(a), Some(b)) => a.iter().filter(|x| b.contains(x)).copied().collect(),
        _ => Vec::new(),
    }
}

impl preprocessor {
    pub fn new(opts: pp_options) -> preprocessor {
        let mut pp = preprocessor {
            opts,
            macros: HashMap::new(),
            stack: Vec::new(),
            includes: Vec::new(),
            conds: Vec::new(),
            once: HashSet::new(),
            lexed: HashMap::new(),
            counter: 0,
            out: Vec::new(),
        };
        pp.predefine();
        pp
    }

    fn predefine(&mut self) {
        let mut text = String::new();
        for (k, v) in Predefined {
            text.push_str(&format!("#define {k} {v}\n"));
        }
        for (k, v) in self.opts.defines.clone() {
            text.push_str(&format!("#define {k} {v}\n"));
        }
        for k in self.opts.undefs.clone() {
            text.push_str(&format!("#undef {k}\n"));
        }
        let id = diag::add_file(
            PathBuf::from("<built-in>"),
            "<built-in>".into(),
            Rc::from(text.as_str()),
            FileKindOther,
        );
        let toks = lex_file(id);
        self.includes.push(include_frame { file_id: id, dir_index: None, cond_depth: 0 });
        self.stack = toks.into_iter().rev().collect();
        self.run();
        self.includes.clear();
        self.out.clear();
        for (name, b) in [
            (SymFileMacro, BuiltinMacroFile),
            (SymLineMacro, BuiltinMacroLine),
            (SymCounterMacro, BuiltinMacroCounter),
            (SymDateMacro, BuiltinMacroDate),
            (SymTimeMacro, BuiltinMacroTime),
        ] {
            let m =
                macro_def { name, params: None, variadic: false, body: vec![], builtin: Some(b) };
            self.macros.insert(name, Rc::new(m));
        }
    }

    /// Preprocesses a whole translation unit.
    pub fn run_file(mut self, file_id: u32) -> Vec<token> {
        let toks = lex_file(file_id);
        let path = diag::file(file_id).path.clone();
        if let Ok(c) = path.canonicalize() {
            self.lexed.insert(c, (file_id, Rc::new(toks.clone())));
        }
        self.includes.push(include_frame { file_id, dir_index: None, cond_depth: 0 });
        self.stack = toks.into_iter().rev().collect();
        self.run();
        let end = self.out.last().map(|t| t.span).unwrap_or_default();
        let mut out = std::mem::take(&mut self.out);
        out.push(token::eof(end));
        out
    }

    fn active(&self) -> bool {
        self.conds.last().is_none_or(|c| c.active)
    }

    fn pop(&mut self) -> token {
        self.stack.pop().unwrap_or_else(|| token::eof(span::dummy()))
    }

    fn peek(&self) -> Option<&token> {
        self.stack.last()
    }

    fn run(&mut self) {
        loop {
            let t = self.pop();
            if t.kind == TokenKindEof {
                if is_sentinel(&t) {
                    diag::ice("argument sentinel reached the top-level preprocessor loop");
                }
                let frame = self.includes.pop();
                if let Some(fr) = frame {
                    while self.conds.len() > fr.cond_depth {
                        let c = self.conds.pop().unwrap();
                        diag::error("CR1003", c.span, "unterminated conditional directive");
                    }
                }
                if self.includes.is_empty() {
                    break;
                }
                continue;
            }
            if t.is(SymHash) && t.has(FlagBol) && !t.has(FlagExpanded) {
                self.directive(t);
                continue;
            }
            if !self.active() {
                continue;
            }
            if t.kind == TokenKindIdent {
                match self.try_expand(t) {
                    Some(t) => self.out.push(t),
                    None => continue,
                }
            } else {
                self.out.push(t);
            }
        }
    }

    /// Reads the rest of the current directive line.
    fn read_line(&mut self) -> Vec<token> {
        let mut v = Vec::new();
        while let Some(t) = self.peek() {
            if t.has(FlagBol) || t.kind == TokenKindEof {
                break;
            }
            v.push(self.pop());
        }
        v
    }

    /// Expands `t` if it names a macro. Returns the token unchanged if it was
    /// not expanded, or `None` if its expansion was pushed back as input.
    fn try_expand(&mut self, mut t: token) -> Option<token> {
        if t.has(FlagNoExpand) {
            return Some(t);
        }
        if hs_contains(&t.hideset, t.text) {
            t.flags |= FlagNoExpand;
            return Some(t);
        }
        if t.text == SymUPragma {
            if self.peek().is_some_and(|n| n.is(SymLParen)) {
                self.pop();
                let s = self.pop();
                let r = self.pop();
                if s.kind != TokenKindString || !r.is(SymRParen) {
                    diag::error("CR1002", t.span, "`_Pragma` takes one string literal");
                }
                return None;
            }
            return Some(t);
        }
        let Some(m) = self.macros.get(&t.text).cloned() else {
            return Some(t);
        };
        if let Some(b) = m.builtin {
            let nt = self.builtin_macro(b, &t);
            return Some(nt);
        }
        let (body, hs) = match &m.params {
            None => {
                let hs = hs_union(&t.hideset, &[m.name]);
                (self.subst(&m, &[], &hs), hs)
            }
            Some(_) => {
                if !self.peek().is_some_and(|n| n.is(SymLParen)) {
                    return Some(t);
                }
                self.pop();
                let (args, rparen) = self.read_args(&m, &t)?;
                let mut hs = hs_intersect(&t.hideset, &rparen.hideset);
                hs.push(m.name);
                let hs = Some(Rc::new(hs));
                (self.subst(&m, &args, &hs), hs)
            }
        };
        let _ = hs;
        let mut body = body;
        for (i, b) in body.iter_mut().enumerate() {
            b.span = t.span;
            b.flags &= !FlagBol;
            b.flags |= FlagExpanded;
            if i == 0 {
                b.flags = (b.flags & !FlagSpace) | (t.flags & FlagSpace);
            }
        }
        for b in body.into_iter().rev() {
            self.stack.push(b);
        }
        None
    }

    fn builtin_macro(&mut self, b: BuiltinMacro, t: &token) -> token {
        let text = match b {
            BuiltinMacroFile => {
                let f = self.includes.last().map(|f| diag::file(f.file_id).display.clone());
                format!("\"{}\"", f.unwrap_or_default().replace('\\', "\\\\"))
            }
            BuiltinMacroLine => {
                let line =
                    if t.span.is_dummy() { 0 } else { diag::line_col(t.span.file, t.span.lo).0 };
                line.to_string()
            }
            BuiltinMacroCounter => {
                self.counter += 1;
                (self.counter - 1).to_string()
            }
            BuiltinMacroDate => "\"Jan  1 2026\"".to_string(),
            BuiltinMacroTime => "\"00:00:00\"".to_string(),
        };
        let kind = if text.starts_with('"') { TokenKindString } else { TokenKindNumber };
        token { kind, text: sym::intern(&text), span: t.span, flags: t.flags, hideset: None }
    }

    /// Reads the arguments of a function-like macro call. The `(` is consumed.
    fn read_args(&mut self, m: &macro_def, name: &token) -> Option<(Vec<Vec<token>>, token)> {
        let mut args: Vec<Vec<token>> = vec![Vec::new()];
        let mut depth = 0;
        let nparams = m.params.as_ref().map(|p| p.len()).unwrap_or(0);
        let rparen;
        loop {
            let t = self.pop();
            if t.kind == TokenKindEof {
                self.stack.push(t);
                diag::error(
                    "CR1004",
                    name.span,
                    format!("unterminated call to macro `{}`", m.name),
                );
                return None;
            }
            if t.is(SymHash) && t.has(FlagBol) && !t.has(FlagExpanded) {
                // A directive inside macro arguments. Process it in place.
                self.directive(t);
                continue;
            }
            if t.is(SymLParen) {
                depth += 1;
            } else if t.is(SymRParen) {
                if depth == 0 {
                    rparen = t;
                    break;
                }
                depth -= 1;
            } else if t.is(SymComma) && depth == 0 && !(m.variadic && args.len() > nparams - 1) {
                args.push(Vec::new());
                continue;
            }
            args.last_mut().unwrap().push(t);
        }
        if nparams == 0 && args.len() == 1 && args[0].is_empty() {
            args.clear();
        }
        if m.variadic && args.len() == nparams - 1 {
            args.push(Vec::new());
        }
        if args.len() != nparams {
            diag::error(
                "CR1004",
                name.span,
                format!(
                    "macro `{}` takes {} argument(s), but {} were given",
                    m.name,
                    nparams,
                    args.len()
                ),
            );
            return None;
        }
        Some((args, rparen))
    }

    /// Fully macro-expands a token list on its own (C23 6.10.5.2).
    fn expand_list(&mut self, toks: &[token]) -> Vec<token> {
        let saved_stack = std::mem::take(&mut self.stack);
        let saved_out = std::mem::take(&mut self.out);
        self.stack.push(sentinel());
        for t in toks.iter().rev() {
            self.stack.push(t.clone());
        }
        loop {
            let t = self.pop();
            if is_sentinel(&t) || t.kind == TokenKindEof {
                break;
            }
            if t.kind == TokenKindIdent {
                if let Some(t) = self.try_expand(t) {
                    self.out.push(t);
                }
            } else {
                self.out.push(t);
            }
        }
        let r = std::mem::replace(&mut self.out, saved_out);
        self.stack = saved_stack;
        r
    }

    fn param_index(m: &macro_def, t: &token) -> Option<usize> {
        if t.kind != TokenKindIdent {
            return None;
        }
        let params = m.params.as_ref()?;
        if m.variadic && t.text == SymVaArgs {
            return Some(params.len() - 1);
        }
        params.iter().position(|p| *p == t.text)
    }

    fn subst(&mut self, m: &macro_def, args: &[Vec<token>], hs: &hideset) -> Vec<token> {
        let body = &m.body;
        let mut out: Vec<token> = Vec::new();
        let mut i = 0;
        while i < body.len() {
            let t = &body[i];
            // `#` param: stringize.
            if t.is(SymHash) && m.params.is_some() {
                if let Some(p) = body.get(i + 1).and_then(|n| Self::param_index(m, n)) {
                    out.push(stringize(&args[p], t));
                    i += 2;
                    continue;
                }
            }
            // `__VA_OPT__ ( ... )`
            if m.variadic && t.is_ident_named(SymVaOpt) {
                let (content, next) = va_opt_content(body, i);
                let va = &args[args.len() - 1];
                let present = !self.expand_list(va).is_empty();
                if present {
                    let sub = macro_def {
                        name: m.name,
                        params: m.params.clone(),
                        variadic: true,
                        body: content,
                        builtin: None,
                    };
                    let mut r = self.subst(&sub, args, hs);
                    out.append(&mut r);
                } else {
                    out.push(placemarker(t));
                }
                i = next;
                continue;
            }
            // `##`: paste with the next operand.
            if t.is(SymHashHash) && !out.is_empty() {
                i += 1;
                let Some(rhs) = body.get(i) else { break };
                let rhs_toks: Vec<token> = match Self::param_index(m, rhs) {
                    Some(p) => args[p].clone(),
                    None => vec![rhs.clone()],
                };
                i += 1;
                let lhs = out.pop().unwrap();
                if rhs_toks.is_empty() {
                    out.push(lhs);
                    continue;
                }
                let pasted = paste(&lhs, &rhs_toks[0]);
                out.push(pasted);
                out.extend(rhs_toks[1..].iter().cloned());
                continue;
            }
            if let Some(p) = Self::param_index(m, t) {
                let next_is_paste = body.get(i + 1).is_some_and(|n| n.is(SymHashHash));
                let arg = if next_is_paste { args[p].clone() } else { self.expand_list(&args[p]) };
                if arg.is_empty() {
                    if next_is_paste {
                        out.push(placemarker(t));
                    }
                } else {
                    let mut first = true;
                    for mut a in arg {
                        if first {
                            a.flags = (a.flags & !FlagSpace) | (t.flags & FlagSpace);
                            first = false;
                        }
                        out.push(a);
                    }
                }
                i += 1;
                continue;
            }
            out.push(t.clone());
            i += 1;
        }
        out.retain(|t| !(t.kind == TokenKindOther && t.text == SymEmpty));
        for t in out.iter_mut() {
            t.hideset = hs_union(&t.hideset, hs.as_deref().map(|v| v.as_slice()).unwrap_or(&[]));
        }
        out
    }

    // ---- directives ----

    fn directive(&mut self, hash: token) {
        let name = if self.peek().is_some_and(|t| !t.has(FlagBol) && t.kind != TokenKindEof) {
            self.pop()
        } else {
            return; // null directive
        };
        let n = name.text;
        let is_cond = matches!(
            n,
            SymIf | SymIfdef | SymIfndef | SymElif | SymElifdef | SymElifndef | SymElse | SymEndif
        );
        if !self.active() && !is_cond {
            self.read_line();
            return;
        }
        match n {
            SymDefine => self.define(),
            SymUndef => {
                let line = self.read_line();
                if let Some(t) = line.first() {
                    self.macros.remove(&t.text);
                }
            }
            SymInclude => self.include(&hash, false),
            SymIncludeNext => self.include(&hash, true),
            SymEmbed => self.embed(&hash),
            SymIf => {
                let line = self.read_line();
                let parent = self.active();
                let v = parent && self.eval_cond(&line, &name);
                self.conds.push(cond_frame {
                    active: v,
                    taken: v,
                    seen_else: false,
                    parent_active: parent,
                    span: name.span,
                });
            }
            SymIfdef | SymIfndef => {
                let line = self.read_line();
                let parent = self.active();
                let def = line.first().is_some_and(|t| self.macros.contains_key(&t.text));
                let v = parent && (def == (n == SymIfdef));
                self.conds.push(cond_frame {
                    active: v,
                    taken: v,
                    seen_else: false,
                    parent_active: parent,
                    span: name.span,
                });
            }
            SymElif | SymElifdef | SymElifndef => {
                let line = self.read_line();
                let Some(c) = self.conds.last() else {
                    diag::error("CR1003", name.span, format!("#{n} without #if"));
                    return;
                };
                if c.seen_else {
                    diag::error("CR1003", name.span, format!("#{n} after #else"));
                }
                let (parent, taken) = (c.parent_active, c.taken);
                let v = if !parent || taken {
                    false
                } else if n == SymElif {
                    self.eval_cond(&line, &name)
                } else {
                    let def = line.first().is_some_and(|t| self.macros.contains_key(&t.text));
                    def == (n == SymElifdef)
                };
                let c = self.conds.last_mut().unwrap();
                c.active = v;
                c.taken |= v;
            }
            SymElse => {
                self.read_line();
                let Some(c) = self.conds.last_mut() else {
                    diag::error("CR1003", name.span, "#else without #if");
                    return;
                };
                if c.seen_else {
                    diag::error("CR1003", name.span, "#else after #else");
                }
                c.seen_else = true;
                c.active = c.parent_active && !c.taken;
                c.taken = true;
            }
            SymEndif => {
                self.read_line();
                let depth = self.includes.last().map(|f| f.cond_depth).unwrap_or(0);
                if self.conds.len() <= depth {
                    diag::error("CR1003", name.span, "#endif without #if");
                } else {
                    self.conds.pop();
                }
            }
            SymLine => {
                self.read_line();
            }
            SymError | SymWarning => {
                let line = self.read_line();
                let msg = spell(&line);
                if n == SymError {
                    diag::error("CR1001", name.span, format!("#error {msg}"));
                } else {
                    diag::warning("CR1001", name.span, format!("#warning {msg}"));
                }
            }
            SymPragma => {
                let line = self.read_line();
                if line.first().is_some_and(|t| t.text == SymOnce) {
                    if let Some(f) = self.includes.last() {
                        if let Ok(p) = diag::file(f.file_id).path.canonicalize() {
                            self.once.insert(p);
                        }
                    }
                }
            }
            _ => {
                if name.kind == TokenKindNumber {
                    // `# 33 "file"` line markers.
                    self.read_line();
                    return;
                }
                self.read_line();
                diag::error("CR1002", name.span, format!("unknown preprocessing directive `#{n}`"));
            }
        }
    }

    fn define(&mut self) {
        let line = self.read_line();
        let Some(name) = line.first() else {
            diag::error("CR1002", span::dummy(), "#define needs a macro name");
            return;
        };
        if name.kind != TokenKindIdent {
            diag::error("CR1002", name.span, "macro name must be an identifier");
            return;
        }
        if name.text == SymDefined {
            diag::error("CR1002", name.span, "`defined` cannot be used as a macro name");
            return;
        }
        let mut i = 1;
        let mut params = None;
        let mut variadic = false;
        if line.get(1).is_some_and(|t| t.is(SymLParen) && !t.has(FlagSpace)) {
            let mut ps = Vec::new();
            i = 2;
            loop {
                let Some(t) = line.get(i) else {
                    diag::error("CR1002", name.span, "unterminated macro parameter list");
                    return;
                };
                i += 1;
                if t.is(SymRParen) {
                    break;
                }
                if t.is(SymComma) {
                    continue;
                }
                if t.is(SymEllipsis) {
                    variadic = true;
                    ps.push(SymVaArgs);
                    continue;
                }
                if t.kind == TokenKindIdent {
                    if line.get(i).is_some_and(|n| n.is(SymEllipsis)) {
                        // GNU named variadic parameter: treat the name as __VA_ARGS__.
                        variadic = true;
                        i += 1;
                    }
                    ps.push(t.text);
                    continue;
                }
                diag::error("CR1002", t.span, "invalid macro parameter");
                return;
            }
            params = Some(ps);
        }
        let body: Vec<token> = line[i..].to_vec();
        let m = macro_def { name: name.text, params, variadic, body, builtin: None };
        self.macros.insert(name.text, Rc::new(m));
    }

    /// Every search directory after the including file's own directory:
    /// `-iquote` dirs, `-I` dirs, the built-in headers, then system dirs.
    fn next_list(&self) -> Vec<SearchDir> {
        let mut v = Vec::new();
        for d in self.opts.quote_dirs.iter().chain(self.opts.include_dirs.iter()) {
            v.push(SearchDirPath(d.clone()));
        }
        v.push(SearchDirBuiltin);
        for d in &self.opts.system_dirs {
            v.push(SearchDirPath(d.clone()));
        }
        v
    }

    fn header_name(&mut self, line: Vec<token>, hash: &token) -> Option<(String, bool, span)> {
        let line = if line.first().is_some_and(|t| t.kind == TokenKindString || t.is(SymLt)) {
            line
        } else {
            self.expand_list(&line)
        };
        let first = line.first()?;
        if first.kind == TokenKindString {
            let s = first.text.as_str();
            return Some((s[1..s.len() - 1].to_string(), true, first.span));
        }
        if first.is(SymLt) {
            let mut name = String::new();
            for t in &line[1..] {
                if t.is(SymGt) {
                    return Some((name, false, first.span));
                }
                if t.has(FlagSpace) && !name.is_empty() {
                    name.push(' ');
                }
                name.push_str(t.text.as_str());
            }
        }
        diag::error("CR1005", hash.span, "expected a header name after #include");
        None
    }

    fn find_include(
        &self,
        name: &str,
        quoted: bool,
        next: bool,
    ) -> Option<(PathBuf, Option<usize>)> {
        if Path::new(name).is_absolute() {
            let p = PathBuf::from(name);
            return p.is_file().then_some((p, None));
        }
        let list = self.next_list();
        let start = if next {
            self.includes.last().and_then(|f| f.dir_index).map(|c| c + 1).unwrap_or(0)
        } else if quoted {
            if let Some(f) = self.includes.last() {
                let cur = diag::file(f.file_id).path.clone();
                if let Some(d) = cur.parent() {
                    let p = d.join(name);
                    if p.is_file() && !cur.starts_with("<crust>") {
                        return Some((p, None));
                    }
                }
            }
            0
        } else {
            self.opts.quote_dirs.len()
        };
        for (idx, d) in list.iter().enumerate().skip(start) {
            match d {
                SearchDirPath(dir) => {
                    let p = dir.join(name);
                    if p.is_file() {
                        return Some((p, Some(idx)));
                    }
                }
                SearchDirBuiltin => {
                    if builtin_headers::get(name).is_some() {
                        return Some((PathBuf::from(format!("<crust>/{name}")), Some(idx)));
                    }
                }
            }
        }
        None
    }

    fn include(&mut self, hash: &token, next: bool) {
        let line = self.read_line();
        let Some((name, quoted, sp)) = self.header_name(line, hash) else { return };
        let Some((path, dir_index)) = self.find_include(&name, quoted, next) else {
            diag::error("CR1005", sp, format!("header `{name}` not found"));
            return;
        };
        let kind = FileKind::from_path(&path);
        if kind == FileKindCrm {
            diag::error("CR1401", sp, format!("cannot `#include` the module file `{name}`"))
                .help("modules are imported, not included: write `import { ... } from \"<module name>\";`");
            return;
        }
        if self.includes.len() > 200 {
            diag::error("CR1005", sp, "#include nested too deeply");
            return;
        }
        let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
        if self.once.contains(&canon) {
            return;
        }
        let (file_id, toks) = match self.lexed.get(&canon) {
            Some((id, toks)) => (*id, toks.clone()),
            None => {
                let text: Rc<str> =
                    if let Some(t) = path.to_str().and_then(|p| p.strip_prefix("<crust>/")) {
                        Rc::from(builtin_headers::get(t).unwrap())
                    } else {
                        match std::fs::read(&path) {
                            Ok(b) => Rc::from(String::from_utf8_lossy(&b).as_ref()),
                            Err(e) => {
                                diag::error(
                                    "CR1005",
                                    sp,
                                    format!("cannot read `{}`: {e}", path.display()),
                                );
                                return;
                            }
                        }
                    };
                let kind = if kind == FileKindOther { crate::diag::FileKindH } else { kind };
                let id = diag::add_file(path.clone(), path.display().to_string(), text, kind);
                let toks = Rc::new(lex_file(id));
                self.lexed.insert(canon.clone(), (id, toks.clone()));
                (id, toks)
            }
        };
        if let Some(guard) = include_guard(&toks) {
            if self.macros.contains_key(&guard) {
                return;
            }
        }
        self.includes.push(include_frame { file_id, dir_index, cond_depth: self.conds.len() });
        for t in toks.iter().rev() {
            self.stack.push(t.clone());
        }
    }

    fn embed(&mut self, hash: &token) {
        let line = self.read_line();
        let Some((name, quoted, sp)) = self.header_name(line.clone(), hash) else { return };
        let Some((path, _)) = self.find_include(&name, quoted, false) else {
            diag::error("CR1005", sp, format!("embedded resource `{name}` not found"));
            return;
        };
        let mut bytes = std::fs::read(&path).unwrap_or_default();
        let mut i = line
            .iter()
            .position(|t| t.is(SymGt) || t.kind == TokenKindString)
            .map(|p| p + 1)
            .unwrap_or(1);
        let (mut prefix, mut suffix, mut if_empty) = (Vec::new(), Vec::new(), None);
        while i < line.len() {
            let p = line[i].text;
            let (content, next) = paren_group(&line, i + 1);
            match p {
                SymLimit => {
                    let e = self.expand_list(&content);
                    let n = cond_expr::eval(&e, &self.macros).max(0) as usize;
                    bytes.truncate(n);
                }
                SymPrefix => prefix = content,
                SymSuffix => suffix = content,
                SymIfEmpty => if_empty = Some(content),
                _ => {}
            }
            i = next;
        }
        let mut toks = Vec::new();
        if bytes.is_empty() {
            toks = if_empty.unwrap_or_default();
        } else {
            toks.extend(prefix);
            for (k, b) in bytes.iter().enumerate() {
                if k > 0 {
                    toks.push(token {
                        kind: TokenKindPunct,
                        text: SymComma,
                        span: hash.span,
                        flags: 0,
                        hideset: None,
                    });
                }
                toks.push(token {
                    kind: TokenKindNumber,
                    text: sym::intern(&b.to_string()),
                    span: hash.span,
                    flags: FlagSpace,
                    hideset: None,
                });
            }
            toks.extend(suffix);
        }
        for t in toks.into_iter().rev() {
            self.stack.push(t);
        }
    }

    fn eval_cond(&mut self, line: &[token], at: &token) -> bool {
        // Resolve `defined` and `__has_*` before macro expansion.
        let mut v: Vec<token> = Vec::new();
        let mut i = 0;
        while i < line.len() {
            let t = &line[i];
            if t.kind == TokenKindIdent && t.text == SymDefined {
                let (name, next) = if line.get(i + 1).is_some_and(|n| n.is(SymLParen)) {
                    (line.get(i + 2).map(|n| n.text), i + 4)
                } else {
                    (line.get(i + 1).map(|n| n.text), i + 2)
                };
                let d = name.is_some_and(|n| self.macros.contains_key(&n));
                v.push(number_token(d as i64, t));
                i = next;
                continue;
            }
            if t.kind == TokenKindIdent
                && matches!(t.text, SymHasInclude | SymHasIncludeNext | SymHasEmbed)
            {
                let (content, next) = paren_group(line, i + 1);
                let r = match self.header_name(content, t) {
                    Some((name, quoted, _)) => {
                        let found = self.find_include(&name, quoted, t.text == SymHasIncludeNext);
                        match (found, t.text == SymHasEmbed) {
                            (None, _) => 0,
                            (Some((p, _)), true) => {
                                if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) == 0 {
                                    2
                                } else {
                                    1
                                }
                            }
                            (Some(_), false) => 1,
                        }
                    }
                    None => 0,
                };
                v.push(number_token(r, t));
                i = next;
                continue;
            }
            if t.kind == TokenKindIdent
                && matches!(
                    t.text,
                    SymHasCAttribute
                        | SymHasAttribute
                        | SymHasBuiltin
                        | SymHasFeature
                        | SymHasExtension
                )
            {
                let (content, next) = paren_group(line, i + 1);
                let r = if t.text == SymHasCAttribute { has_c_attribute(&content) } else { 0 };
                v.push(number_token(r, t));
                i = next;
                continue;
            }
            v.push(t.clone());
            i += 1;
        }
        let e = self.expand_list(&v);
        if e.is_empty() {
            diag::error("CR1002", at.span, "#if with no expression");
            return false;
        }
        cond_expr::eval(&e, &self.macros) != 0
    }
}

fn number_token(v: i64, at: &token) -> token {
    token {
        kind: TokenKindNumber,
        text: sym::intern(&v.to_string()),
        span: at.span,
        flags: FlagSpace,
        hideset: None,
    }
}

fn has_c_attribute(content: &[token]) -> i64 {
    let names: Vec<sym> =
        content.iter().filter(|t| t.kind == TokenKindIdent).map(|t| t.text).collect();
    let n = names.last().map(|s| s.as_str()).unwrap_or("");
    let n = n.trim_start_matches("__").trim_end_matches("__");
    if names.first() == Some(&SymCrust) {
        return matches!(n, "drop" | "safe" | "c_abi") as i64;
    }
    match n {
        "deprecated" => 201904,
        "fallthrough" => 201904,
        "maybe_unused" => 201904,
        "nodiscard" => 202003,
        "noreturn" | "_Noreturn" => 202202,
        "unsequenced" | "reproducible" => 202207,
        _ => 0,
    }
}

/// Tokens between the parentheses that start at `i`, and the index after them.
fn paren_group(line: &[token], i: usize) -> (Vec<token>, usize) {
    if !line.get(i).is_some_and(|t| t.is(SymLParen)) {
        return (Vec::new(), i);
    }
    let mut depth = 0;
    let mut j = i;
    while j < line.len() {
        if line[j].is(SymLParen) {
            depth += 1;
        } else if line[j].is(SymRParen) {
            depth -= 1;
            if depth == 0 {
                return (line[i + 1..j].to_vec(), j + 1);
            }
        }
        j += 1;
    }
    (line[i + 1..].to_vec(), line.len())
}

fn va_opt_content(body: &[token], i: usize) -> (Vec<token>, usize) {
    paren_group(body, i + 1)
}

fn placemarker(at: &token) -> token {
    token { kind: TokenKindOther, text: SymEmpty, span: at.span, flags: 0, hideset: None }
}

fn spell(toks: &[token]) -> String {
    let mut s = String::new();
    for (i, t) in toks.iter().enumerate() {
        if i > 0 && t.has(FlagSpace) {
            s.push(' ');
        }
        s.push_str(t.text.as_str());
    }
    s
}

fn stringize(arg: &[token], at: &token) -> token {
    let mut s = String::from("\"");
    for (i, t) in arg.iter().enumerate() {
        if i > 0 && t.has(FlagSpace) {
            s.push(' ');
        }
        let text = t.text.as_str();
        if matches!(t.kind, TokenKindString | TokenKindChar) {
            for ch in text.chars() {
                if ch == '"' || ch == '\\' {
                    s.push('\\');
                }
                s.push(ch);
            }
        } else {
            s.push_str(text);
        }
    }
    s.push('"');
    token {
        kind: TokenKindString,
        text: sym::intern(&s),
        span: at.span,
        flags: at.flags,
        hideset: None,
    }
}

fn paste(lhs: &token, rhs: &token) -> token {
    if lhs.kind == TokenKindOther && lhs.text == SymEmpty {
        return rhs.clone();
    }
    if rhs.kind == TokenKindOther && rhs.text == SymEmpty {
        return lhs.clone();
    }
    let text = format!("{}{}", lhs.text, rhs.text);
    let kind = classify_spelling(&text).unwrap_or_else(|| {
        diag::error(
            "CR1006",
            lhs.span,
            format!("pasting `{}` and `{}` does not give a valid token", lhs.text, rhs.text),
        );
        TokenKindOther
    });
    token {
        kind,
        text: sym::intern(&text),
        span: lhs.span,
        flags: lhs.flags,
        hideset: lhs.hideset.clone(),
    }
}

fn classify_spelling(s: &str) -> Option<TokenKind> {
    let b = s.as_bytes();
    if b.is_empty() {
        return None;
    }
    if (b[0].is_ascii_alphabetic() || b[0] == b'_' || b[0] >= 0x80)
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c >= 0x80)
    {
        return Some(TokenKindIdent);
    }
    if b[0].is_ascii_digit() || (b[0] == b'.' && b.len() > 1 && b[1].is_ascii_digit()) {
        return Some(TokenKindNumber);
    }
    if s.ends_with('"') {
        return Some(TokenKindString);
    }
    if Puncts2.contains(&s) {
        return Some(TokenKindPunct);
    }
    None
}

const Puncts2: &[&str] = &[
    "...", "<<=", ">>=", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "::",
    "*=", "/=", "%=", "+=", "-=", "&=", "^=", "|=", "##", "(", ")", "[", "]", "{", "}", ".", "&",
    "*", "+", "-", "~", "!", "/", "%", "<", ">", "^", "|", "?", ":", ";", "=", ",", "#",
];

/// Detects the `#ifndef G / #define G ... #endif` include-guard pattern.
fn include_guard(toks: &[token]) -> Option<sym> {
    let t = |i: usize| toks.get(i);
    if !(t(0)?.is(SymHash) && t(1)?.text == SymIfndef && t(2)?.kind == TokenKindIdent) {
        return None;
    }
    let guard = t(2)?.text;
    if !(t(3)?.is(SymHash) && t(3)?.has(FlagBol) && t(4)?.text == SymDefine && t(5)?.text == guard)
    {
        return None;
    }
    // The matching #endif must be the last directive in the file.
    let n = toks.len();
    if n < 3 {
        return None;
    }
    let last = &toks[n - 2];
    let hash = &toks[n - 3];
    if !(last.text == SymEndif && hash.is(SymHash) && hash.has(FlagBol)) {
        return None;
    }
    let mut depth = 0i32;
    for (k, x) in toks.iter().enumerate() {
        if x.is(SymHash) && x.has(FlagBol) {
            match toks.get(k + 1).map(|y| y.text) {
                Some(SymIf | SymIfdef | SymIfndef) => depth += 1,
                Some(SymEndif) => {
                    depth -= 1;
                    if depth == 0 && k != n - 3 {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }
    Some(guard)
}

/// Predefined macros for x86-64 Linux. `__GNUC__` is not defined, so system
/// headers fall back to standard C.
const Predefined: &[(&str, &str)] = &[
    ("__STDC__", "1"),
    ("__STDC_VERSION__", "202311L"),
    ("__STDC_HOSTED__", "1"),
    ("__STDC_UTF_16__", "1"),
    ("__STDC_UTF_32__", "1"),
    ("__STDC_NO_ATOMICS__", "1"),
    ("__STDC_NO_COMPLEX__", "1"),
    ("__STDC_NO_VLA__", "1"),
    ("__STDC_EMBED_NOT_FOUND__", "0"),
    ("__STDC_EMBED_FOUND__", "1"),
    ("__STDC_EMBED_EMPTY__", "2"),
    ("__x86_64__", "1"),
    ("__x86_64", "1"),
    ("__amd64__", "1"),
    ("__amd64", "1"),
    ("__linux__", "1"),
    ("__linux", "1"),
    ("__gnu_linux__", "1"),
    ("__unix__", "1"),
    ("__unix", "1"),
    ("__ELF__", "1"),
    ("__LP64__", "1"),
    ("_LP64", "1"),
    ("__CHAR_BIT__", "8"),
    ("__SIZEOF_SHORT__", "2"),
    ("__SIZEOF_INT__", "4"),
    ("__SIZEOF_LONG__", "8"),
    ("__SIZEOF_LONG_LONG__", "8"),
    ("__SIZEOF_POINTER__", "8"),
    ("__SIZEOF_FLOAT__", "4"),
    ("__SIZEOF_DOUBLE__", "8"),
    ("__SIZEOF_LONG_DOUBLE__", "16"),
    ("__SIZEOF_SIZE_T__", "8"),
    ("__SIZEOF_WCHAR_T__", "4"),
    ("__SIZEOF_WINT_T__", "4"),
    ("__SIZEOF_PTRDIFF_T__", "8"),
    ("__SIZE_TYPE__", "unsigned long"),
    ("__PTRDIFF_TYPE__", "long"),
    ("__WCHAR_TYPE__", "int"),
    ("__WINT_TYPE__", "unsigned int"),
    ("__INTMAX_TYPE__", "long"),
    ("__UINTMAX_TYPE__", "unsigned long"),
    ("__INTPTR_TYPE__", "long"),
    ("__UINTPTR_TYPE__", "unsigned long"),
    ("__CHAR16_TYPE__", "unsigned short"),
    ("__CHAR32_TYPE__", "unsigned int"),
    ("__SCHAR_MAX__", "127"),
    ("__SHRT_MAX__", "32767"),
    ("__INT_MAX__", "2147483647"),
    ("__LONG_MAX__", "9223372036854775807L"),
    ("__LONG_LONG_MAX__", "9223372036854775807LL"),
    ("__WCHAR_MAX__", "2147483647"),
    ("__WCHAR_MIN__", "(-2147483647 - 1)"),
    ("__SIZE_MAX__", "18446744073709551615UL"),
    ("__PTRDIFF_MAX__", "9223372036854775807L"),
    ("__INTMAX_MAX__", "9223372036854775807L"),
    ("__UINTMAX_MAX__", "18446744073709551615UL"),
    ("__INTPTR_MAX__", "9223372036854775807L"),
    ("__ORDER_LITTLE_ENDIAN__", "1234"),
    ("__ORDER_BIG_ENDIAN__", "4321"),
    ("__ORDER_PDP_ENDIAN__", "3412"),
    ("__BYTE_ORDER__", "__ORDER_LITTLE_ENDIAN__"),
    ("__FLT_EVAL_METHOD__", "0"),
    ("__FLT_RADIX__", "2"),
];

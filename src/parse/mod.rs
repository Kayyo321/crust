/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
pub mod ast;
pub mod literal;

use crate::diag::{self, FileKind, FileKindCrm, span};
use crate::lex::*;
use ast::*;
use std::collections::HashMap;
use std::rc::Rc;

/// What the parser needs to know about imported modules: which imported
/// names are typedefs, so declarations parse correctly.
pub trait import_oracle {
    fn is_typedef(&self, spec: &str, name: sym) -> bool;
}

pub struct no_imports;

impl import_oracle for no_imports {
    fn is_typedef(&self, _spec: &str, _name: sym) -> bool {
        false
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum DeclMode {
    DeclModeNamed,
    DeclModeAbstract,
    DeclModeEither,
}
use DeclMode::*;

pub struct parser<'a> {
    toks: Vec<token>,
    pos: usize,
    /// Ordinary-identifier scopes. `true` marks a typedef name.
    scopes: Vec<HashMap<sym, bool>>,
    namespaces: HashMap<sym, String>,
    oracle: &'a dyn import_oracle,
    file: u32,
    kind: FileKind,
    /// Suppresses cascading errors until the parser resynchronizes.
    recovering: bool,
}

pub fn parse_translation_unit(
    toks: Vec<token>,
    file: u32,
    oracle: &dyn import_oracle,
) -> translation_unit {
    let kind = diag::file_kind(file);
    let mut p = parser {
        toks,
        pos: 0,
        scopes: vec![HashMap::new()],
        namespaces: HashMap::new(),
        oracle,
        file,
        kind,
        recovering: false,
    };
    p.declare(SymBuiltinVaList, true);
    p.declare(SymInt128T, true);
    p.declare(SymUInt128T, true);
    p.translation_unit()
}

fn is_assign_op(s: sym) -> Option<Option<BinOp>> {
    Some(match s {
        SymAssign => None,
        SymStarEq => Some(BinOpMul),
        SymSlashEq => Some(BinOpDiv),
        SymPercentEq => Some(BinOpRem),
        SymPlusEq => Some(BinOpAdd),
        SymMinusEq => Some(BinOpSub),
        SymShlEq => Some(BinOpShl),
        SymShrEq => Some(BinOpShr),
        SymAmpEq => Some(BinOpBitAnd),
        SymCaretEq => Some(BinOpBitXor),
        SymPipeEq => Some(BinOpBitOr),
        _ => return None,
    })
}

fn binop_prec(s: sym) -> Option<(BinOp, u8)> {
    Some(match s {
        SymPipePipe => (BinOpLogOr, 1),
        SymAmpAmp => (BinOpLogAnd, 2),
        SymPipe => (BinOpBitOr, 3),
        SymCaret => (BinOpBitXor, 4),
        SymAmp => (BinOpBitAnd, 5),
        SymEqEq => (BinOpEq, 6),
        SymNe => (BinOpNe, 6),
        SymLt => (BinOpLt, 7),
        SymGt => (BinOpGt, 7),
        SymLe => (BinOpLe, 7),
        SymGe => (BinOpGe, 7),
        SymShl => (BinOpShl, 8),
        SymShr => (BinOpShr, 8),
        SymPlus => (BinOpAdd, 9),
        SymMinus => (BinOpSub, 9),
        SymStar => (BinOpMul, 10),
        SymSlash => (BinOpDiv, 10),
        SymPercent => (BinOpRem, 10),
        _ => return None,
    })
}

fn is_type_keyword(s: sym) -> bool {
    matches!(
        s,
        SymVoid
            | SymChar
            | SymShort
            | SymInt
            | SymLong
            | SymFloat
            | SymDouble
            | SymSigned
            | SymUnsigned
            | SymBool
            | SymStruct
            | SymUnion
            | SymEnum
            | SymTypeof
            | SymTypeofUnqual
            | SymUAtomic
            | SymUComplex
            | SymUImaginary
            | SymInt128
            | SymBuiltinVaList
            | SymUBitInt
            | SymUDecimal32
            | SymUDecimal64
            | SymUDecimal128
    )
}

fn is_qualifier_keyword(s: sym) -> bool {
    matches!(s, SymConst | SymVolatile | SymRestrict | SymUAtomic)
}

fn is_decl_keyword(s: sym) -> bool {
    is_type_keyword(s)
        || is_qualifier_keyword(s)
        || matches!(
            s,
            SymTypedef
                | SymExtern
                | SymStatic
                | SymAuto
                | SymRegister
                | SymInline
                | SymUNoreturn
                | SymConstexpr
                | SymThreadLocal
                | SymAlignas
                | SymGnuAttribute
                | SymGnuExtension
        )
}

impl parser<'_> {
    // ---- token access ----

    fn peek(&self) -> &token {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn peek_at(&self, n: usize) -> &token {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)]
    }

    fn bump(&mut self) -> token {
        let t = self.peek().clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at(&self, s: sym) -> bool {
        self.peek().is(s)
    }

    fn at_eof(&self) -> bool {
        self.peek().kind == TokenKindEof
    }

    fn eat(&mut self, s: sym) -> bool {
        if self.at(s) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn prev_span(&self) -> span {
        if self.pos == 0 { self.peek().span } else { self.toks[self.pos - 1].span }
    }

    fn describe(t: &token) -> String {
        match t.kind {
            TokenKindEof => "end of file".into(),
            TokenKindString => "a string literal".into(),
            TokenKindNumber => format!("`{}`", t.text),
            _ => format!("`{}`", t.text),
        }
    }

    fn error_expected(&mut self, what: &str) {
        if self.recovering {
            return;
        }
        self.recovering = true;
        let t = self.peek().clone();
        diag::error("CR1201", t.span, format!("expected {what}, found {}", Self::describe(&t)));
    }

    fn expect(&mut self, s: sym) -> span {
        if self.at(s) {
            return self.bump().span;
        }
        self.error_expected(&format!("`{s}`"));
        self.prev_span()
    }

    /// Skips to just after the next `;` at this nesting level, or to a `}`
    /// that closes the current block.
    fn synchronize(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            if self.at(SymLBrace) || self.at(SymLParen) || self.at(SymLBracket) {
                depth += 1;
            } else if self.at(SymRParen) || self.at(SymRBracket) {
                depth -= 1;
            } else if self.at(SymRBrace) {
                if depth <= 0 {
                    break;
                }
                depth -= 1;
                if depth == 0 {
                    self.bump();
                    if self.at(SymSemi) {
                        self.bump();
                    }
                    break;
                }
            } else if self.at(SymSemi) && depth <= 0 {
                self.bump();
                break;
            }
            self.bump();
        }
        self.recovering = false;
    }

    // ---- scopes ----

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: sym, is_typedef: bool) {
        self.scopes.last_mut().unwrap().insert(name, is_typedef);
    }

    fn is_typedef(&self, name: sym) -> bool {
        for s in self.scopes.iter().rev() {
            if let Some(&t) = s.get(&name) {
                return t;
            }
        }
        false
    }

    fn is_namespace(&self, t: &token) -> bool {
        t.kind == TokenKindIdent && self.namespaces.contains_key(&t.text)
    }

    /// `ns::Name` where Name is a typedef exported by the namespace's module.
    fn at_qualified_typedef(&self) -> bool {
        let t = self.peek();
        if !(self.is_namespace(t) && self.peek_at(1).is(SymColonColon)) {
            return false;
        }
        let name = self.peek_at(2);
        let spec = &self.namespaces[&t.text];
        name.kind == TokenKindIdent && self.oracle.is_typedef(spec, name.text)
    }

    fn token_starts_type(&self, n: usize) -> bool {
        let t = self.peek_at(n);
        match t.kind {
            TokenKindKeyword => {
                is_type_keyword(t.text)
                    || is_qualifier_keyword(t.text)
                    || t.text == SymAlignas
                    || t.text == SymGnuAttribute
            }
            TokenKindIdent => {
                if self.is_typedef(t.text) {
                    return true;
                }
                self.is_namespace(t)
                    && self.peek_at(n + 1).is(SymColonColon)
                    && self.oracle.is_typedef(&self.namespaces[&t.text], self.peek_at(n + 2).text)
            }
            _ => false,
        }
    }

    /// Whether `ref` at offset `n` starts a type rather than a borrow expression.
    fn ref_starts_type(&self, n: usize) -> bool {
        let mut k = n + 1;
        if self.peek_at(k).kind == TokenKindLifetime {
            return true;
        }
        if self.peek_at(k).is(SymMut) {
            k += 1;
        }
        self.peek_at(k).is(SymLBracket)
            || self.token_starts_type(k)
            || (self.peek_at(k).is(SymRef) && self.ref_starts_type(k))
    }

    /// Whether a type name starts at offset `n` (used after `(`).
    fn starts_type_name(&self, n: usize) -> bool {
        if self.peek_at(n).is(SymRef) {
            return self.ref_starts_type(n);
        }
        self.token_starts_type(n)
    }

    fn starts_declaration(&self) -> bool {
        let t = self.peek();
        match t.kind {
            TokenKindKeyword => {
                if t.text == SymUnsafe {
                    return !self.peek_at(1).is(SymLBrace);
                }
                if t.text == SymRef {
                    return self.ref_starts_type(0);
                }
                if t.text == SymGnuExtension {
                    return true;
                }
                is_decl_keyword(t.text)
            }
            TokenKindIdent => {
                if self.peek_at(1).is(SymColon) {
                    return false;
                }
                self.token_starts_type(0)
            }
            TokenKindPunct => t.is(SymLBracket) && self.peek_at(1).is(SymLBracket),
            _ => false,
        }
    }

    // ---- translation unit ----

    fn translation_unit(&mut self) -> translation_unit {
        let mut module = None;
        if self.at(SymModule) {
            let start = self.bump().span;
            let (name, sp) = self.dotted_name();
            self.expect(SymSemi);
            if self.kind != FileKindCrm {
                diag::error("CR1403", start.to(sp), "only a `.crm` file can declare a module")
                    .help("rename the file to `.crm`, or remove the `module` declaration");
            }
            module = Some((name, sp));
        } else if self.kind == FileKindCrm {
            let sp = self.peek().span;
            diag::error("CR1402", sp, "a `.crm` module must start with `module <name>;`")
                .help("add a module declaration, for example `module app;`, before any other code");
        }
        let mut items = Vec::new();
        while !self.at_eof() {
            let before = self.pos;
            if let Some(it) = self.external_item() {
                items.push(it);
            }
            if self.recovering {
                self.synchronize();
            }
            if self.pos == before {
                self.bump();
            }
        }
        translation_unit { file: self.file, kind: self.kind, module, items }
    }

    fn dotted_name(&mut self) -> (sym, span) {
        let mut s = String::new();
        let start = self.peek().span;
        let mut end = start;
        loop {
            let t = self.peek().clone();
            if t.kind != TokenKindIdent && t.kind != TokenKindKeyword {
                self.error_expected("a module name");
                break;
            }
            self.bump();
            end = t.span;
            s.push_str(t.text.as_str());
            if self.at(SymDot) {
                self.bump();
                s.push('.');
            } else {
                break;
            }
        }
        (sym::intern(&s), start.to(end))
    }

    fn external_item(&mut self) -> Option<item> {
        let start = self.peek().span;
        if self.eat(SymSemi) {
            return None;
        }
        if self.at(SymModule) {
            diag::error("CR1402", start, "`module` must be the first declaration in the file");
            self.recovering = true;
            return None;
        }
        if self.at(SymImport) {
            let d = self.import_decl();
            let sp = d.span;
            return Some(item { kind: ItemKindImport(d), span: sp, exported: None });
        }
        let attrs = self.attribute_specifiers();
        let mut exported = None;
        if self.at(SymExport) {
            exported = Some(self.bump().span);
            if self.at(SymLBrace) || self.at(SymStar) {
                let l = self.export_list(start);
                let sp = l.span;
                return Some(item { kind: ItemKindExportList(l), span: sp, exported });
            }
        }
        let mut attrs = attrs;
        attrs.extend(self.attribute_specifiers());
        if self.at(SymLet) {
            let l = self.let_decl(attrs);
            let sp = l.span;
            return Some(item { kind: ItemKindLet(l), span: sp, exported });
        }
        if self.at(SymStaticAssert) {
            let s = self.static_assert_decl();
            let sp = s.span;
            return Some(item { kind: ItemKindStaticAssert(s), span: sp, exported });
        }
        let kind = self.external_declaration(attrs)?;
        Some(item { kind, span: start.to(self.prev_span()), exported })
    }

    fn string_spec(&mut self) -> (String, span) {
        let t = self.peek().clone();
        if t.kind != TokenKindString {
            self.error_expected("a module specifier string");
            return (String::new(), t.span);
        }
        self.bump();
        let s = t.text.as_str();
        (s[1..s.len() - 1].to_string(), t.span)
    }

    fn import_names(&mut self) -> Vec<import_name> {
        let mut names = Vec::new();
        self.expect(SymLBrace);
        while !self.at(SymRBrace) && !self.at_eof() {
            let t = self.peek().clone();
            if t.kind != TokenKindIdent {
                self.error_expected("an imported name");
                break;
            }
            self.bump();
            let mut alias = None;
            let mut sp = t.span;
            if self.peek().is_ident_named(SymAs) {
                self.bump();
                let a = self.peek().clone();
                if a.kind != TokenKindIdent {
                    self.error_expected("a name after `as`");
                    break;
                }
                self.bump();
                alias = Some(a.text);
                sp = sp.to(a.span);
            }
            names.push(import_name { name: t.text, alias, span: sp });
            if !self.eat(SymComma) {
                break;
            }
        }
        self.expect(SymRBrace);
        names
    }

    fn expect_from(&mut self) -> (String, span) {
        if self.peek().is_ident_named(SymFrom) {
            self.bump();
        } else {
            self.error_expected("`from`");
        }
        self.string_spec()
    }

    fn import_decl(&mut self) -> import_decl {
        let start = self.bump().span;
        let kind = if self.eat(SymStar) {
            if self.peek().is_ident_named(SymAs) {
                self.bump();
            } else {
                self.error_expected("`as`");
            }
            let t = self.bump();
            if t.kind != TokenKindIdent {
                self.error_expected("a namespace name");
            }
            ImportKindNamespace(t.text, t.span)
        } else if self.at(SymLBrace) {
            ImportKindNamed(self.import_names())
        } else if self.peek().kind == TokenKindString {
            let sp = self.peek().span;
            diag::error("CR1408", start.to(sp), "side-effect imports are not supported")
                .note("importing a module runs no code (docs/MODULES.md, section 11)");
            self.bump();
            self.expect(SymSemi);
            return import_decl {
                kind: ImportKindNamed(vec![]),
                from: String::new(),
                from_span: sp,
                span: start.to(sp),
            };
        } else {
            let sp = self.peek().span;
            if self.peek().kind == TokenKindIdent {
                diag::error("CR1408", sp, "default imports are not supported")
                    .help("import names explicitly: `import { name } from \"module\";`");
            } else {
                self.error_expected("`{` or `*` after `import`");
            }
            self.recovering = true;
            return import_decl {
                kind: ImportKindNamed(vec![]),
                from: String::new(),
                from_span: sp,
                span: start,
            };
        };
        let (from, from_span) = self.expect_from();
        self.expect(SymSemi);
        match &kind {
            ImportKindNamed(names) => {
                for n in names {
                    let is_t = self.oracle.is_typedef(&from, n.name);
                    self.declare(n.alias.unwrap_or(n.name), is_t);
                }
            }
            ImportKindNamespace(alias, _) => {
                self.namespaces.insert(*alias, from.clone());
            }
        }
        import_decl { kind, from, from_span, span: start.to(self.prev_span()) }
    }

    fn export_list(&mut self, start: span) -> export_list {
        if self.eat(SymStar) {
            let from = self.expect_from();
            self.expect(SymSemi);
            return export_list {
                names: vec![],
                star: true,
                from: Some(from),
                span: start.to(self.prev_span()),
            };
        }
        let names = self.import_names();
        let from =
            if self.peek().is_ident_named(SymFrom) { Some(self.expect_from()) } else { None };
        self.expect(SymSemi);
        export_list { names, star: false, from, span: start.to(self.prev_span()) }
    }

    fn external_declaration(&mut self, attrs: Vec<attribute>) -> Option<ItemKind> {
        let start = self.peek().span;
        let mut spec = self.decl_spec(true)?;
        spec.attrs.splice(0..0, attrs);
        if self.at(SymSemi) {
            self.bump();
            return Some(ItemKindDecl(declaration {
                spec,
                inits: vec![],
                span: start.to(self.prev_span()),
            }));
        }
        let mut d = self.declarator(DeclModeNamed);
        self.gnu_trailers(&mut d);
        if d.function_params().is_some() && self.at(SymLBrace) {
            let is_typedef = matches!(spec.storage, Some((StorageTypedef, _)));
            if let Some((n, _)) = d.name {
                self.declare(n, is_typedef);
            }
            self.push_scope();
            if let Some((params, _)) = d.function_params() {
                let names: Vec<sym> =
                    params.iter().filter_map(|p| p.declarator.name.map(|n| n.0)).collect();
                for n in names {
                    self.declare(n, false);
                }
            }
            let body = self.compound_stmt(false);
            self.pop_scope();
            let sp = start.to(self.prev_span());
            return Some(ItemKindFunc(function_def { spec, declarator: d, body, span: sp }));
        }
        let decl = self.init_declarator_list(spec, d, start);
        Some(ItemKindDecl(decl))
    }

    fn init_declarator_list(
        &mut self,
        spec: decl_spec,
        first: declarator,
        start: span,
    ) -> declaration {
        let is_typedef = matches!(spec.storage, Some((StorageTypedef, _)));
        let mut inits = Vec::new();
        let mut d = first;
        loop {
            let asm_label = self.gnu_trailers(&mut d);
            if let Some((n, _)) = d.name {
                self.declare(n, is_typedef);
            }
            let init = if self.eat(SymAssign) { Some(self.initializer()) } else { None };
            inits.push(init_declarator { declarator: d, init, asm_label });
            if !self.eat(SymComma) {
                break;
            }
            d = self.declarator(DeclModeNamed);
        }
        if !self.at(SymSemi) {
            self.error_expected("`;` after declaration");
        } else {
            self.bump();
        }
        declaration { spec, inits, span: start.to(self.prev_span()) }
    }

    /// `__asm__("name")` and `__attribute__((...))` after a declarator.
    fn gnu_trailers(&mut self, d: &mut declarator) -> Option<String> {
        let mut label = None;
        loop {
            if self.at(SymGnuAsm) {
                self.bump();
                self.expect(SymLParen);
                let mut s = String::new();
                while self.peek().kind == TokenKindString {
                    let t = self.bump();
                    let txt = t.text.as_str();
                    s.push_str(&txt[1..txt.len() - 1]);
                }
                self.expect(SymRParen);
                label = Some(s);
            } else if self.at(SymGnuAttribute) {
                self.skip_gnu_attribute();
            } else if self.at(SymLBracket) && self.peek_at(1).is(SymLBracket) {
                let a = self.attribute_specifiers();
                d.attrs.extend(a);
            } else {
                break;
            }
        }
        label
    }

    fn skip_gnu_attribute(&mut self) {
        self.bump();
        if !self.at(SymLParen) {
            return;
        }
        let mut depth = 0;
        loop {
            let t = self.bump();
            if t.is(SymLParen) {
                depth += 1;
            } else if t.is(SymRParen) {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            } else if t.kind == TokenKindEof {
                break;
            }
        }
    }

    // ---- attributes ----

    fn attribute_specifiers(&mut self) -> Vec<attribute> {
        let mut out = Vec::new();
        loop {
            if self.at(SymGnuAttribute) {
                self.skip_gnu_attribute();
                continue;
            }
            if !(self.at(SymLBracket) && self.peek_at(1).is(SymLBracket)) {
                break;
            }
            self.bump();
            self.bump();
            while !self.at(SymRBracket) && !self.at_eof() {
                if self.eat(SymComma) {
                    continue;
                }
                let t = self.bump();
                if !matches!(t.kind, TokenKindIdent | TokenKindKeyword) {
                    self.error_expected("an attribute name");
                    break;
                }
                let (prefix, name, mut sp) = if self.at(SymColonColon) {
                    self.bump();
                    let n = self.bump();
                    (Some(t.text), n.text, t.span.to(n.span))
                } else {
                    (None, t.text, t.span)
                };
                let mut args = None;
                if self.at(SymLParen) {
                    self.bump();
                    let mut depth = 1;
                    let mut v = Vec::new();
                    while !self.at_eof() {
                        let a = self.bump();
                        if a.is(SymLParen) {
                            depth += 1;
                        } else if a.is(SymRParen) {
                            depth -= 1;
                            if depth == 0 {
                                sp = sp.to(a.span);
                                break;
                            }
                        }
                        v.push(a);
                    }
                    args = Some(v);
                }
                out.push(attribute { prefix, name, args, span: sp });
            }
            self.expect(SymRBracket);
            self.expect(SymRBracket);
        }
        out
    }

    // ---- declaration specifiers ----

    fn decl_spec(&mut self, allow_storage: bool) -> Option<decl_spec> {
        let start = self.peek().span;
        let mut s = decl_spec {
            storage: None,
            thread_local: false,
            inline_: false,
            noreturn: false,
            constexpr_: false,
            unsafe_: None,
            ty: TypeSpecNone,
            quals: 0,
            attrs: Vec::new(),
            align: Vec::new(),
            span: start,
        };
        let mut counts = builtin_counts::default();
        let mut saw_auto = None;
        loop {
            let t = self.peek().clone();
            if t.is(SymLBracket) && self.peek_at(1).is(SymLBracket) {
                let a = self.attribute_specifiers();
                s.attrs.extend(a);
                continue;
            }
            match t.kind {
                TokenKindKeyword => {}
                TokenKindIdent => {
                    let have_type = !matches!(s.ty, TypeSpecNone) || counts.any();
                    if !have_type && self.is_typedef(t.text) {
                        self.bump();
                        s.ty = TypeSpecTypedefName(t.text, None, t.span);
                        continue;
                    }
                    if !have_type && self.at_qualified_typedef() {
                        self.bump();
                        self.bump();
                        let n = self.bump();
                        s.ty = TypeSpecTypedefName(n.text, Some(t.text), t.span.to(n.span));
                        continue;
                    }
                    break;
                }
                _ => break,
            }
            let k = t.text;
            let storage = match k {
                SymTypedef => Some(StorageTypedef),
                SymExtern => Some(StorageExtern),
                SymStatic => Some(StorageStatic),
                SymRegister => Some(StorageRegister),
                _ => None,
            };
            if let Some(st) = storage {
                self.bump();
                if !allow_storage {
                    diag::error(
                        "CR1202",
                        t.span,
                        format!("storage class `{k}` is not allowed here"),
                    );
                } else if s.storage.is_some() {
                    diag::error("CR1202", t.span, "more than one storage class in a declaration");
                }
                s.storage = Some((st, t.span));
                continue;
            }
            match k {
                SymAuto => {
                    self.bump();
                    saw_auto = Some(t.span);
                }
                SymInline => {
                    self.bump();
                    s.inline_ = true;
                }
                SymUNoreturn => {
                    self.bump();
                    s.noreturn = true;
                }
                SymConstexpr => {
                    self.bump();
                    s.constexpr_ = true;
                }
                SymThreadLocal => {
                    self.bump();
                    s.thread_local = true;
                }
                SymGnuExtension => {
                    self.bump();
                }
                SymGnuAttribute => self.skip_gnu_attribute(),
                SymUnsafe => {
                    if self.peek_at(1).is(SymLBrace) {
                        break;
                    }
                    self.bump();
                    s.unsafe_ = Some(t.span);
                }
                SymConst => {
                    self.bump();
                    s.quals |= QualConst;
                }
                SymVolatile => {
                    self.bump();
                    s.quals |= QualVolatile;
                }
                SymRestrict => {
                    self.bump();
                    s.quals |= QualRestrict;
                }
                SymUAtomic => {
                    self.bump();
                    if self.at(SymLParen) {
                        self.bump();
                        let tn = self.type_name();
                        self.expect(SymRParen);
                        s.ty = TypeSpecAtomic(Box::new(tn));
                    } else {
                        s.quals |= QualAtomic;
                    }
                }
                SymAlignas => {
                    self.bump();
                    self.expect(SymLParen);
                    if self.starts_type_name(0) {
                        let tn = self.type_name();
                        s.align.push(align_spec::AlignType(Box::new(tn)));
                    } else {
                        let e = self.cond_expr();
                        s.align.push(align_spec::AlignExpr(Box::new(e)));
                    }
                    self.expect(SymRParen);
                }
                SymVoid => {
                    self.bump();
                    counts.void += 1;
                }
                SymBool => {
                    self.bump();
                    counts.bool_ += 1;
                }
                SymChar => {
                    self.bump();
                    counts.char_ += 1;
                }
                SymShort => {
                    self.bump();
                    counts.short += 1;
                }
                SymInt => {
                    self.bump();
                    counts.int += 1;
                }
                SymLong => {
                    self.bump();
                    counts.long += 1;
                }
                SymFloat => {
                    self.bump();
                    counts.float += 1;
                }
                SymDouble => {
                    self.bump();
                    counts.double += 1;
                }
                SymSigned => {
                    self.bump();
                    counts.signed += 1;
                }
                SymUnsigned => {
                    self.bump();
                    counts.unsigned += 1;
                }
                SymInt128 => {
                    self.bump();
                    counts.int128 += 1;
                }
                SymUComplex | SymUImaginary => {
                    self.bump();
                    counts.complex += 1;
                }
                SymBuiltinVaList => {
                    self.bump();
                    counts.va_list += 1;
                }
                SymUBitInt | SymUDecimal32 | SymUDecimal64 | SymUDecimal128 => {
                    self.bump();
                    diag::error("CR1301", t.span, format!("`{k}` is not supported in Crust 0.1.0"));
                    if self.at(SymLParen) {
                        self.bump();
                        self.cond_expr();
                        self.expect(SymRParen);
                    }
                    counts.int += 1;
                }
                SymStruct | SymUnion => {
                    let r = self.record_spec();
                    s.ty = TypeSpecRecord(Rc::new(r));
                }
                SymEnum => {
                    let e = self.enum_spec();
                    s.ty = TypeSpecEnum(Rc::new(e));
                }
                SymTypeof | SymTypeofUnqual => {
                    self.bump();
                    let unqual = k == SymTypeofUnqual;
                    self.expect(SymLParen);
                    if self.starts_type_name(0) {
                        let tn = self.type_name();
                        s.ty = TypeSpecTypeofType(Box::new(tn), unqual);
                    } else {
                        let e = self.expr();
                        s.ty = TypeSpecTypeofExpr(Box::new(e), unqual);
                    }
                    self.expect(SymRParen);
                }
                SymRef => {
                    if !matches!(s.ty, TypeSpecNone) || counts.any() || !self.ref_starts_type(0) {
                        break;
                    }
                    s.ty = self.ref_type_spec();
                }
                _ => break,
            }
        }
        if counts.any() {
            if !matches!(s.ty, TypeSpecNone) {
                diag::error("CR1202", start, "conflicting type specifiers");
            }
            s.ty = TypeSpecBuiltin(counts);
        }
        if let Some(sp) = saw_auto {
            if matches!(s.ty, TypeSpecNone) {
                s.ty = TypeSpecAuto;
            } else if s.storage.is_none() {
                s.storage = Some((StorageAuto, sp));
            }
        }
        s.span = start.to(self.prev_span());
        Some(s)
    }

    fn ref_type_spec(&mut self) -> TypeSpec {
        self.bump();
        let lifetime = if self.peek().kind == TokenKindLifetime {
            let t = self.bump();
            Some((t.text, t.span))
        } else {
            None
        };
        let mutable = self.eat(SymMut);
        if self.at(SymLBracket) {
            self.bump();
            let elem = self.type_name();
            self.expect(SymRBracket);
            return TypeSpecSlice { mutable, lifetime, elem: Box::new(elem) };
        }
        let inner = if self.at(SymLParen) {
            self.bump();
            let tn = self.type_name();
            self.expect(SymRParen);
            tn
        } else {
            let start = self.peek().span;
            let spec = self.decl_spec(false).unwrap();
            type_name {
                spec,
                declarator: declarator::abstract_at(start),
                span: start.to(self.prev_span()),
            }
        };
        TypeSpecRef { mutable, lifetime, inner: Box::new(inner) }
    }

    fn tag_name(&mut self) -> (Option<(sym, span)>, Option<sym>) {
        let t = self.peek().clone();
        if t.kind != TokenKindIdent {
            return (None, None);
        }
        if self.is_namespace(&t) && self.peek_at(1).is(SymColonColon) {
            self.bump();
            self.bump();
            let n = self.bump();
            return (Some((n.text, t.span.to(n.span))), Some(t.text));
        }
        self.bump();
        (Some((t.text, t.span)), None)
    }

    fn record_spec(&mut self) -> record_spec {
        let kw = self.bump();
        let is_union = kw.text == SymUnion;
        let mut attrs = self.attribute_specifiers();
        let (tag, ns) = self.tag_name();
        attrs.extend(self.attribute_specifiers());
        let mut fields = None;
        if self.at(SymLBrace) {
            self.bump();
            let mut fs = Vec::new();
            while !self.at(SymRBrace) && !self.at_eof() {
                let before = self.pos;
                if self.eat(SymSemi) {
                    continue;
                }
                if self.at(SymStaticAssert) {
                    let sa = self.static_assert_decl();
                    let sp = sa.span;
                    fs.push(field_decl {
                        spec: self.empty_spec(sp),
                        declarators: vec![],
                        span: sp,
                        static_assert: Some(sa),
                    });
                    continue;
                }
                fs.push(self.field_decl());
                if self.recovering {
                    self.synchronize();
                }
                if self.pos == before {
                    self.bump();
                }
            }
            self.expect(SymRBrace);
            fields = Some(fs);
        }
        attrs.extend(self.attribute_specifiers());
        if tag.is_none() && fields.is_none() {
            self.error_expected("a struct or union tag or `{`");
        }
        record_spec { is_union, tag, ns, fields, attrs, span: kw.span.to(self.prev_span()) }
    }

    fn empty_spec(&self, sp: span) -> decl_spec {
        decl_spec {
            storage: None,
            thread_local: false,
            inline_: false,
            noreturn: false,
            constexpr_: false,
            unsafe_: None,
            ty: TypeSpecNone,
            quals: 0,
            attrs: vec![],
            align: vec![],
            span: sp,
        }
    }

    fn field_decl(&mut self) -> field_decl {
        let start = self.peek().span;
        let spec = self.decl_spec(false).unwrap();
        let mut declarators = Vec::new();
        if !self.at(SymSemi) {
            loop {
                let d = if self.at(SymColon) { None } else { Some(self.declarator(DeclModeNamed)) };
                let width = if self.eat(SymColon) { Some(self.cond_expr()) } else { None };
                let mut d = d;
                if let Some(dd) = d.as_mut() {
                    self.gnu_trailers(dd);
                } else {
                    self.attribute_specifiers();
                }
                declarators.push((d, width));
                if !self.eat(SymComma) {
                    break;
                }
            }
        }
        self.expect(SymSemi);
        field_decl { spec, declarators, span: start.to(self.prev_span()), static_assert: None }
    }

    fn enum_spec(&mut self) -> enum_spec {
        let kw = self.bump();
        let mut attrs = self.attribute_specifiers();
        let (tag, ns) = self.tag_name();
        attrs.extend(self.attribute_specifiers());
        let mut fixed = None;
        if self.at(SymColon) {
            self.bump();
            let start = self.peek().span;
            let spec = self.decl_spec(false).unwrap();
            fixed = Some(Box::new(type_name {
                spec,
                declarator: declarator::abstract_at(start),
                span: start.to(self.prev_span()),
            }));
        }
        let mut enumerators = None;
        if self.at(SymLBrace) {
            self.bump();
            let mut es = Vec::new();
            while !self.at(SymRBrace) && !self.at_eof() {
                let t = self.peek().clone();
                if t.kind != TokenKindIdent {
                    self.error_expected("an enumerator name");
                    break;
                }
                self.bump();
                self.attribute_specifiers();
                let value = if self.eat(SymAssign) { Some(self.cond_expr()) } else { None };
                self.declare(t.text, false);
                es.push(enumerator { name: t.text, span: t.span, value });
                if !self.eat(SymComma) {
                    break;
                }
            }
            self.expect(SymRBrace);
            enumerators = Some(es);
        }
        if tag.is_none() && enumerators.is_none() {
            self.error_expected("an enum tag or `{`");
        }
        enum_spec { tag, ns, fixed, enumerators, attrs, span: kw.span.to(self.prev_span()) }
    }

    // ---- declarators ----

    fn type_qualifiers(&mut self) -> u8 {
        let mut q = 0;
        loop {
            if self.at(SymConst) {
                q |= QualConst;
            } else if self.at(SymVolatile) {
                q |= QualVolatile;
            } else if self.at(SymRestrict) {
                q |= QualRestrict;
            } else if self.at(SymUAtomic) && !self.peek_at(1).is(SymLParen) {
                q |= QualAtomic;
            } else if self.at(SymGnuAttribute) {
                self.skip_gnu_attribute();
                continue;
            } else if self.at(SymLBracket) && self.peek_at(1).is(SymLBracket) {
                self.attribute_specifiers();
                continue;
            } else {
                break;
            }
            self.bump();
        }
        q
    }

    fn declarator(&mut self, mode: DeclMode) -> declarator {
        let start = self.peek().span;
        let mut ptrs = Vec::new();
        while self.at(SymStar) {
            self.bump();
            let quals = self.type_qualifiers();
            ptrs.push(DerivedPointer { quals });
        }
        let mut inner: Option<declarator> = None;
        let mut name = None;
        let mut attrs = Vec::new();
        if self.at(SymLParen) && self.paren_is_nested_declarator(mode) {
            self.bump();
            inner = Some(self.declarator(mode));
            self.expect(SymRParen);
        } else if self.peek().kind == TokenKindIdent && mode != DeclModeAbstract {
            let t = self.bump();
            name = Some((t.text, t.span));
            if self.at(SymLBracket) && self.peek_at(1).is(SymLBracket) {
                attrs = self.attribute_specifiers();
            }
        } else if mode == DeclModeNamed {
            self.error_expected("a declarator name");
        }
        let mut suffixes = Vec::new();
        loop {
            if self.at(SymLBracket) && !self.peek_at(1).is(SymLBracket) {
                let lb = self.bump().span;
                let mut is_static = self.eat(SymStatic);
                let quals = self.type_qualifiers();
                is_static |= self.eat(SymStatic);
                let mut star = false;
                let size = if self.at(SymRBracket) {
                    None
                } else if self.at(SymStar) && self.peek_at(1).is(SymRBracket) {
                    self.bump();
                    star = true;
                    None
                } else {
                    Some(Box::new(self.assign_expr()))
                };
                let rb = self.expect(SymRBracket);
                suffixes.push(DerivedArray { size, is_static, quals, star, span: lb.to(rb) });
            } else if self.at(SymLParen) {
                let lp = self.bump().span;
                let (params, variadic) = self.param_list();
                let rp = self.expect(SymRParen);
                suffixes.push(DerivedFunction { params, variadic, span: lp.to(rp) });
            } else {
                break;
            }
        }
        let mut derived = ptrs;
        derived.extend(suffixes.into_iter().rev());
        if let Some(inn) = inner {
            derived.extend(inn.derived);
            name = inn.name;
            attrs.extend(inn.attrs);
        }
        declarator { name, derived, attrs, span: start.to(self.prev_span()) }
    }

    fn paren_is_nested_declarator(&self, mode: DeclMode) -> bool {
        let n = self.peek_at(1);
        if n.is(SymStar)
            || n.is(SymLParen)
            || (n.is(SymLBracket) && self.peek_at(2).is(SymLBracket))
        {
            // `(*` or `((`: nested, unless it is an abstract function
            // declarator whose first parameter starts with `(`; that cannot
            // start a parameter, so this is nested.
            return true;
        }
        if n.kind == TokenKindIdent && mode != DeclModeAbstract {
            return !self.token_starts_type(1);
        }
        if n.is(SymGnuAttribute) {
            return true;
        }
        false
    }

    fn param_list(&mut self) -> (Vec<param_decl>, bool) {
        let mut params = Vec::new();
        let mut variadic = false;
        if self.at(SymRParen) {
            return (params, false);
        }
        if self.at(SymVoid) && self.peek_at(1).is(SymRParen) {
            self.bump();
            return (params, false);
        }
        loop {
            if self.eat(SymEllipsis) {
                variadic = true;
                break;
            }
            let start = self.peek().span;
            if !self.starts_type_name(0) && !self.at(SymRegister) && !self.at(SymLBracket) {
                if self.peek().kind == TokenKindIdent {
                    diag::error(
                        "CR1203",
                        self.peek().span,
                        format!("unknown type name `{}`", self.peek().text),
                    )
                    .note(
                        "C23 has no old-style (K&R) parameter lists; each parameter needs a type",
                    );
                    self.recovering = true;
                    while !self.at(SymRParen) && !self.at_eof() {
                        self.bump();
                    }
                    break;
                }
                self.error_expected("a parameter declaration");
                break;
            }
            let spec = self.decl_spec(true).unwrap();
            let d = self.declarator(DeclModeEither);
            params.push(param_decl { spec, declarator: d, span: start.to(self.prev_span()) });
            if !self.eat(SymComma) {
                break;
            }
        }
        (params, variadic)
    }

    pub fn type_name(&mut self) -> type_name {
        let start = self.peek().span;
        let spec = self.decl_spec(false).unwrap();
        if matches!(spec.ty, TypeSpecNone) && spec.quals == 0 {
            self.error_expected("a type name");
        }
        let d = self.declarator(DeclModeAbstract);
        type_name { spec, declarator: d, span: start.to(self.prev_span()) }
    }

    // ---- initializers ----

    fn initializer(&mut self) -> Initializer {
        if !self.at(SymLBrace) {
            return InitializerExpr(Box::new(self.assign_expr()));
        }
        let lb = self.bump().span;
        let mut items = Vec::new();
        while !self.at(SymRBrace) && !self.at_eof() {
            let mut desig = Vec::new();
            loop {
                if self.at(SymDot) {
                    let d = self.bump().span;
                    let n = self.bump();
                    if n.kind != TokenKindIdent {
                        self.error_expected("a member name");
                    }
                    desig.push(DesignatorField(n.text, d.to(n.span)));
                } else if self.at(SymLBracket) {
                    let d = self.bump().span;
                    let e = self.cond_expr();
                    let r = self.expect(SymRBracket);
                    desig.push(DesignatorIndex(Box::new(e), d.to(r)));
                } else {
                    break;
                }
            }
            if !desig.is_empty() {
                self.expect(SymAssign);
            }
            let init = self.initializer();
            items.push((desig, init));
            if !self.eat(SymComma) {
                break;
            }
        }
        let rb = self.expect(SymRBrace);
        InitializerList(items, lb.to(rb))
    }

    // ---- Crust declarations ----

    fn let_decl(&mut self, attrs: Vec<attribute>) -> let_decl {
        let start = self.bump().span;
        let mutable = self.eat(SymMut);
        let t = self.peek().clone();
        if t.kind != TokenKindIdent {
            self.error_expected("a name after `let`");
            return let_decl {
                mutable,
                name: SymEmpty,
                name_span: t.span,
                ty: None,
                init: None,
                attrs,
                span: start,
            };
        }
        self.bump();
        let ty = if self.eat(SymColon) { Some(Box::new(self.type_name())) } else { None };
        self.declare(t.text, false);
        let init = if self.eat(SymAssign) { Some(self.initializer()) } else { None };
        if !self.at(SymSemi) {
            self.error_expected("`;` after `let` declaration");
        } else {
            self.bump();
        }
        let_decl {
            mutable,
            name: t.text,
            name_span: t.span,
            ty,
            init,
            attrs,
            span: start.to(self.prev_span()),
        }
    }

    fn static_assert_decl(&mut self) -> static_assert {
        let start = self.bump().span;
        self.expect(SymLParen);
        let cond = self.cond_expr();
        let mut message = None;
        if self.eat(SymComma) {
            let mut s = String::new();
            while self.peek().kind == TokenKindString {
                let t = self.bump();
                let txt = t.text.as_str();
                s.push_str(&txt[txt.find('"').unwrap() + 1..txt.len() - 1]);
            }
            message = Some(s);
        }
        self.expect(SymRParen);
        self.expect(SymSemi);
        static_assert { cond: Box::new(cond), message, span: start.to(self.prev_span()) }
    }

    // ---- statements ----

    fn compound_stmt(&mut self, new_scope: bool) -> stmt {
        let lb = self.expect(SymLBrace);
        if new_scope {
            self.push_scope();
        }
        let items = self.block_items();
        if new_scope {
            self.pop_scope();
        }
        let rb = self.expect(SymRBrace);
        stmt { kind: StmtKindCompound(items), span: lb.to(rb) }
    }

    fn block_items(&mut self) -> Vec<BlockItem> {
        let mut items = Vec::new();
        while !self.at(SymRBrace) && !self.at_eof() {
            let before = self.pos;
            if let Some(it) = self.block_item() {
                items.push(it);
            }
            if self.recovering {
                self.synchronize();
            }
            if self.pos == before {
                self.bump();
            }
        }
        items
    }

    fn block_item(&mut self) -> Option<BlockItem> {
        if self.at(SymLet) {
            return Some(BlockItemLet(self.let_decl(vec![])));
        }
        if self.at(SymStaticAssert) {
            return Some(BlockItemStaticAssert(self.static_assert_decl()));
        }
        if self.starts_declaration() {
            let start = self.peek().span;
            let attrs = self.attribute_specifiers();
            if self.at(SymLet) {
                return Some(BlockItemLet(self.let_decl(attrs)));
            }
            if !self.starts_declaration() {
                // Attributes on a statement (for example [[fallthrough]]).
                return Some(BlockItemStmt(self.statement()));
            }
            let mut spec = self.decl_spec(true)?;
            spec.attrs.splice(0..0, attrs);
            if self.at(SymSemi) {
                self.bump();
                return Some(BlockItemDecl(declaration {
                    spec,
                    inits: vec![],
                    span: start.to(self.prev_span()),
                }));
            }
            let d = self.declarator(DeclModeNamed);
            return Some(BlockItemDecl(self.init_declarator_list(spec, d, start)));
        }
        Some(BlockItemStmt(self.statement()))
    }

    fn statement(&mut self) -> stmt {
        let start = self.peek().span;
        let attrs = self.attribute_specifiers();
        let _ = attrs;
        let t = self.peek().clone();
        if t.kind == TokenKindIdent && self.peek_at(1).is(SymColon) {
            self.bump();
            self.bump();
            let body = self.label_body();
            return stmt {
                kind: StmtKindLabeled(LabelKindNamed(t.text), t.span, Box::new(body)),
                span: start.to(self.prev_span()),
            };
        }
        match t.text {
            _ if t.kind != TokenKindKeyword && t.kind != TokenKindPunct => {}
            SymLBrace if t.kind == TokenKindPunct => return self.compound_stmt(true),
            SymSemi if t.kind == TokenKindPunct => {
                self.bump();
                return stmt { kind: StmtKindExpr(None), span: t.span };
            }
            SymCase => {
                self.bump();
                let e = self.cond_expr();
                let c = self.expect(SymColon);
                let body = self.label_body();
                return stmt {
                    kind: StmtKindLabeled(LabelKindCase(Box::new(e)), t.span.to(c), Box::new(body)),
                    span: start.to(self.prev_span()),
                };
            }
            SymDefault => {
                self.bump();
                let c = self.expect(SymColon);
                let body = self.label_body();
                return stmt {
                    kind: StmtKindLabeled(LabelKindDefault, t.span.to(c), Box::new(body)),
                    span: start.to(self.prev_span()),
                };
            }
            SymIf => {
                self.bump();
                self.expect(SymLParen);
                let c = self.expr();
                self.expect(SymRParen);
                let then = self.sub_statement();
                let els =
                    if self.eat(SymElse) { Some(Box::new(self.sub_statement())) } else { None };
                return stmt {
                    kind: StmtKindIf(c, Box::new(then), els),
                    span: start.to(self.prev_span()),
                };
            }
            SymWhile => {
                self.bump();
                self.expect(SymLParen);
                let c = self.expr();
                self.expect(SymRParen);
                let body = self.sub_statement();
                return stmt {
                    kind: StmtKindWhile(c, Box::new(body)),
                    span: start.to(self.prev_span()),
                };
            }
            SymDo => {
                self.bump();
                let body = self.sub_statement();
                self.expect(SymWhile);
                self.expect(SymLParen);
                let c = self.expr();
                self.expect(SymRParen);
                self.expect(SymSemi);
                return stmt {
                    kind: StmtKindDoWhile(Box::new(body), c),
                    span: start.to(self.prev_span()),
                };
            }
            SymFor => return self.for_stmt(),
            SymSwitch => {
                self.bump();
                self.expect(SymLParen);
                let c = self.expr();
                self.expect(SymRParen);
                let body = self.sub_statement();
                return stmt {
                    kind: StmtKindSwitch(c, Box::new(body)),
                    span: start.to(self.prev_span()),
                };
            }
            SymBreak => {
                self.bump();
                self.expect(SymSemi);
                return stmt { kind: StmtKindBreak, span: start.to(self.prev_span()) };
            }
            SymContinue => {
                self.bump();
                self.expect(SymSemi);
                return stmt { kind: StmtKindContinue, span: start.to(self.prev_span()) };
            }
            SymReturn => {
                self.bump();
                let e = if self.at(SymSemi) { None } else { Some(self.expr()) };
                self.expect(SymSemi);
                return stmt { kind: StmtKindReturn(e), span: start.to(self.prev_span()) };
            }
            SymGoto => {
                self.bump();
                let n = self.bump();
                if n.kind != TokenKindIdent {
                    self.error_expected("a label name");
                }
                self.expect(SymSemi);
                return stmt { kind: StmtKindGoto(n.text), span: start.to(self.prev_span()) };
            }
            SymUnsafe if self.peek_at(1).is(SymLBrace) => {
                self.bump();
                self.expect(SymLBrace);
                self.push_scope();
                let items = self.block_items();
                self.pop_scope();
                self.expect(SymRBrace);
                return stmt { kind: StmtKindUnsafe(items), span: start.to(self.prev_span()) };
            }
            _ => {}
        }
        let e = self.expr();
        if !self.at(SymSemi) {
            self.error_expected("`;`");
        } else {
            self.bump();
        }
        stmt { kind: StmtKindExpr(Some(e)), span: start.to(self.prev_span()) }
    }

    /// The statement after a label. C23 allows a label at the end of a block
    /// and before a declaration.
    fn label_body(&mut self) -> stmt {
        if self.at(SymRBrace) {
            return stmt { kind: StmtKindExpr(None), span: self.prev_span() };
        }
        if self.starts_declaration() || self.at(SymLet) {
            let sp = self.peek().span;
            return stmt { kind: StmtKindExpr(None), span: sp };
        }
        self.statement()
    }

    /// A statement that is the body of a selection or iteration statement,
    /// with its own block scope.
    fn sub_statement(&mut self) -> stmt {
        self.push_scope();
        let s = self.statement();
        self.pop_scope();
        s
    }

    fn for_stmt(&mut self) -> stmt {
        let start = self.bump().span;
        self.expect(SymLParen);
        self.push_scope();
        let init = if self.eat(SymSemi) {
            None
        } else if self.at(SymLet) {
            Some(Box::new(BlockItemLet(self.let_decl(vec![]))))
        } else if self.starts_declaration() {
            let s = self.peek().span;
            let spec = self.decl_spec(true).unwrap();
            let d = self.declarator(DeclModeNamed);
            Some(Box::new(BlockItemDecl(self.init_declarator_list(spec, d, s))))
        } else {
            let e = self.expr();
            let sp = e.span;
            self.expect(SymSemi);
            Some(Box::new(BlockItemStmt(stmt { kind: StmtKindExpr(Some(e)), span: sp })))
        };
        let cond = if self.at(SymSemi) { None } else { Some(self.expr()) };
        self.expect(SymSemi);
        let step = if self.at(SymRParen) { None } else { Some(self.expr()) };
        self.expect(SymRParen);
        let body = self.sub_statement();
        self.pop_scope();
        stmt {
            kind: StmtKindFor(init, cond, step, Box::new(body)),
            span: start.to(self.prev_span()),
        }
    }

    // ---- expressions ----

    pub fn expr(&mut self) -> expr {
        let mut e = self.assign_expr();
        while self.at(SymComma) {
            self.bump();
            let r = self.assign_expr();
            let sp = e.span.to(r.span);
            e = expr { kind: ExprKindComma(Box::new(e), Box::new(r)), span: sp };
        }
        e
    }

    pub fn assign_expr(&mut self) -> expr {
        let lhs = self.cond_expr();
        let t = self.peek();
        if t.kind == TokenKindPunct {
            if let Some(op) = is_assign_op(t.text) {
                self.bump();
                let rhs = self.assign_expr();
                let sp = lhs.span.to(rhs.span);
                return expr { kind: ExprKindAssign(op, Box::new(lhs), Box::new(rhs)), span: sp };
            }
        }
        lhs
    }

    pub fn cond_expr(&mut self) -> expr {
        let c = self.binary_expr(0);
        if !self.at(SymQuestion) {
            return c;
        }
        self.bump();
        let t = self.expr();
        self.expect(SymColon);
        let e = self.cond_expr();
        let sp = c.span.to(e.span);
        expr { kind: ExprKindCond(Box::new(c), Box::new(t), Box::new(e)), span: sp }
    }

    fn binary_expr(&mut self, min: u8) -> expr {
        let mut lhs = self.cast_expr();
        loop {
            let t = self.peek();
            if t.kind != TokenKindPunct {
                break;
            }
            let Some((op, p)) = binop_prec(t.text) else { break };
            if p <= min {
                break;
            }
            self.bump();
            let rhs = self.binary_expr(p);
            let sp = lhs.span.to(rhs.span);
            lhs = expr { kind: ExprKindBinary(op, Box::new(lhs), Box::new(rhs)), span: sp };
        }
        lhs
    }

    fn cast_expr(&mut self) -> expr {
        if self.at(SymLParen) && self.starts_type_name(1) {
            let lp = self.bump().span;
            let tn = self.type_name();
            self.expect(SymRParen);
            if self.at(SymLBrace) {
                let init = self.initializer();
                let sp = lp.to(self.prev_span());
                let e = expr { kind: ExprKindCompound(Box::new(tn), init), span: sp };
                return self.postfix(e);
            }
            let operand = self.cast_expr();
            let sp = lp.to(operand.span);
            return expr { kind: ExprKindCast(Box::new(tn), Box::new(operand)), span: sp };
        }
        self.unary_expr()
    }

    fn unary_expr(&mut self) -> expr {
        let t = self.peek().clone();
        let op = match t.text {
            _ if t.kind != TokenKindPunct && t.kind != TokenKindKeyword => None,
            SymPlusPlus => Some(UnOpPreInc),
            SymMinusMinus => Some(UnOpPreDec),
            SymAmp => Some(UnOpAddrOf),
            SymStar => Some(UnOpDeref),
            SymPlus => Some(UnOpPlus),
            SymMinus => Some(UnOpNeg),
            SymTilde => Some(UnOpBitNot),
            SymBang => Some(UnOpNot),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let e = if matches!(op, UnOpPreInc | UnOpPreDec) {
                self.unary_expr()
            } else {
                self.cast_expr()
            };
            let sp = t.span.to(e.span);
            return expr { kind: ExprKindUnary(op, Box::new(e)), span: sp };
        }
        if t.is(SymRef) {
            self.bump();
            let mutable = self.eat(SymMut);
            let e = self.cast_expr();
            let sp = t.span.to(e.span);
            return expr { kind: ExprKindRef(mutable, Box::new(e)), span: sp };
        }
        if t.is(SymSizeof) {
            self.bump();
            if self.at(SymLParen) && self.starts_type_name(1) {
                self.bump();
                let tn = self.type_name();
                let rp = self.expect(SymRParen);
                if self.at(SymLBrace) {
                    let init = self.initializer();
                    let sp = t.span.to(self.prev_span());
                    let lit = expr { kind: ExprKindCompound(Box::new(tn), init), span: sp };
                    let lit = self.postfix(lit);
                    return expr { kind: ExprKindSizeofExpr(Box::new(lit)), span: sp };
                }
                return expr { kind: ExprKindSizeofType(Box::new(tn)), span: t.span.to(rp) };
            }
            let e = self.unary_expr();
            let sp = t.span.to(e.span);
            return expr { kind: ExprKindSizeofExpr(Box::new(e)), span: sp };
        }
        if t.is(SymAlignof) {
            self.bump();
            if self.at(SymLParen) && self.starts_type_name(1) {
                self.bump();
                let tn = self.type_name();
                let rp = self.expect(SymRParen);
                return expr { kind: ExprKindAlignofType(Box::new(tn)), span: t.span.to(rp) };
            }
            let e = self.unary_expr();
            let sp = t.span.to(e.span);
            return expr { kind: ExprKindAlignofExpr(Box::new(e)), span: sp };
        }
        if t.is(SymGnuExtension) {
            self.bump();
            return self.cast_expr();
        }
        let p = self.primary_expr();
        self.postfix(p)
    }

    fn postfix(&mut self, mut e: expr) -> expr {
        loop {
            let t = self.peek().clone();
            if t.kind != TokenKindPunct {
                break;
            }
            match t.text {
                SymLBracket => {
                    self.bump();
                    let i = self.expr();
                    let rb = self.expect(SymRBracket);
                    let sp = e.span.to(rb);
                    e = expr { kind: ExprKindIndex(Box::new(e), Box::new(i)), span: sp };
                }
                SymLParen => {
                    self.bump();
                    let mut args = Vec::new();
                    while !self.at(SymRParen) && !self.at_eof() {
                        args.push(self.assign_expr());
                        if !self.eat(SymComma) {
                            break;
                        }
                    }
                    let rp = self.expect(SymRParen);
                    let sp = e.span.to(rp);
                    e = expr { kind: ExprKindCall(Box::new(e), args), span: sp };
                }
                SymDot | SymArrow => {
                    self.bump();
                    let n = self.peek().clone();
                    if n.kind != TokenKindIdent && n.kind != TokenKindKeyword {
                        self.error_expected("a member name");
                        break;
                    }
                    self.bump();
                    let sp = e.span.to(n.span);
                    e = expr {
                        kind: ExprKindMember {
                            base: Box::new(e),
                            name: n.text,
                            name_span: n.span,
                            arrow: t.text == SymArrow,
                        },
                        span: sp,
                    };
                }
                SymPlusPlus | SymMinusMinus => {
                    self.bump();
                    let op = if t.text == SymPlusPlus { UnOpPostInc } else { UnOpPostDec };
                    let sp = e.span.to(t.span);
                    e = expr { kind: ExprKindUnary(op, Box::new(e)), span: sp };
                }
                _ => break,
            }
        }
        e
    }

    fn primary_expr(&mut self) -> expr {
        let t = self.peek().clone();
        match t.kind {
            TokenKindIdent => {
                self.bump();
                if self.at(SymColonColon) && self.is_namespace(&t) {
                    self.bump();
                    let n = self.bump();
                    if n.kind != TokenKindIdent {
                        self.error_expected("a name after `::`");
                    }
                    return expr {
                        kind: ExprKindQualified(t.text, n.text),
                        span: t.span.to(n.span),
                    };
                }
                expr { kind: ExprKindIdent(t.text), span: t.span }
            }
            TokenKindNumber => {
                self.bump();
                let kind = if literal::is_float_spelling(t.text.as_str()) {
                    ExprKindFloat(t.text)
                } else {
                    ExprKindInt(t.text)
                };
                expr { kind, span: t.span }
            }
            TokenKindChar => {
                self.bump();
                expr { kind: ExprKindChar(t.text), span: t.span }
            }
            TokenKindString => {
                let mut pieces = Vec::new();
                let mut sp = t.span;
                while self.peek().kind == TokenKindString {
                    let s = self.bump();
                    sp = sp.to(s.span);
                    pieces.push(s.text);
                }
                expr { kind: ExprKindString(pieces), span: sp }
            }
            TokenKindKeyword => match t.text {
                SymTrue | SymFalse => {
                    self.bump();
                    expr { kind: ExprKindBool(t.text == SymTrue), span: t.span }
                }
                SymNullptr => {
                    self.bump();
                    expr { kind: ExprKindNullptr, span: t.span }
                }
                SymUGeneric => self.generic_selection(),
                SymBuiltinVaArg => {
                    self.bump();
                    self.expect(SymLParen);
                    let e = self.assign_expr();
                    self.expect(SymComma);
                    let tn = self.type_name();
                    let rp = self.expect(SymRParen);
                    expr { kind: ExprKindVaArg(Box::new(e), Box::new(tn)), span: t.span.to(rp) }
                }
                SymBuiltinOffsetof => {
                    self.bump();
                    self.expect(SymLParen);
                    let tn = self.type_name();
                    self.expect(SymComma);
                    let mut path = Vec::new();
                    let first = self.bump();
                    path.push(DesignatorField(first.text, first.span));
                    loop {
                        if self.at(SymDot) {
                            self.bump();
                            let n = self.bump();
                            path.push(DesignatorField(n.text, n.span));
                        } else if self.at(SymLBracket) {
                            let lb = self.bump().span;
                            let e = self.expr();
                            let rb = self.expect(SymRBracket);
                            path.push(DesignatorIndex(Box::new(e), lb.to(rb)));
                        } else {
                            break;
                        }
                    }
                    let rp = self.expect(SymRParen);
                    expr { kind: ExprKindOffsetof(Box::new(tn), path), span: t.span.to(rp) }
                }
                _ => {
                    self.error_expected("an expression");
                    expr { kind: ExprKindInt(sym::intern("0")), span: t.span }
                }
            },
            TokenKindPunct if t.is(SymLParen) => {
                self.bump();
                let e = self.expr();
                let rp = self.expect(SymRParen);
                expr { kind: e.kind, span: t.span.to(rp) }
            }
            TokenKindOther => {
                self.bump();
                diag::error("CR1103", t.span, format!("stray `{}` in program", t.text));
                self.recovering = true;
                expr { kind: ExprKindInt(sym::intern("0")), span: t.span }
            }
            _ => {
                self.error_expected("an expression");
                expr { kind: ExprKindInt(sym::intern("0")), span: t.span }
            }
        }
    }

    fn generic_selection(&mut self) -> expr {
        let start = self.bump().span;
        self.expect(SymLParen);
        let ctrl = self.assign_expr();
        let mut assocs = Vec::new();
        while self.eat(SymComma) {
            let tn = if self.eat(SymDefault) { None } else { Some(self.type_name()) };
            self.expect(SymColon);
            let e = self.assign_expr();
            assocs.push((tn, e));
        }
        let rp = self.expect(SymRParen);
        expr { kind: ExprKindGeneric(Box::new(ctrl), assocs), span: start.to(rp) }
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
pub mod symbols;

use crate::diag::{self, span};
use std::rc::Rc;
pub use symbols::*;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TokenKind {
    TokenKindIdent,
    TokenKindKeyword,
    TokenKindNumber,
    TokenKindChar,
    TokenKindString,
    TokenKindPunct,
    TokenKindLifetime,
    TokenKindOther,
    TokenKindEof,
}
pub use TokenKind::*;

/// First token on its line.
pub const FlagBol: u8 = 1;
/// Whitespace before the token.
pub const FlagSpace: u8 = 2;
/// Spelled in a Crust file, so Crust keywords apply.
pub const FlagCrust: u8 = 4;
/// Never macro-expand this identifier again.
pub const FlagNoExpand: u8 = 8;
/// Produced by a macro expansion.
pub const FlagExpanded: u8 = 16;

/// The set of macro names a token must not be expanded by again.
pub type hideset = Option<Rc<Vec<sym>>>;

#[derive(Clone, Debug)]
pub struct token {
    pub kind: TokenKind,
    pub text: sym,
    pub span: span,
    pub flags: u8,
    pub hideset: hideset,
}

impl token {
    pub fn is(&self, s: sym) -> bool {
        self.text == s && matches!(self.kind, TokenKindPunct | TokenKindKeyword)
    }

    pub fn is_ident(&self) -> bool {
        self.kind == TokenKindIdent
    }

    pub fn is_ident_named(&self, s: sym) -> bool {
        self.kind == TokenKindIdent && self.text == s
    }

    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    pub fn eof(sp: span) -> token {
        token { kind: TokenKindEof, text: SymEmpty, span: sp, flags: FlagBol, hideset: None }
    }
}

const Puncts: &[&str] = &[
    "%:%:", "...", "<<=", ">>=", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||",
    "::", "*=", "/=", "%=", "+=", "-=", "&=", "^=", "|=", "##", "<:", ":>", "<%", "%>", "%:", "(",
    ")", "[", "]", "{", "}", ".", "&", "*", "+", "-", "~", "!", "/", "%", "<", ">", "^", "|", "?",
    ":", ";", "=", ",", "#",
];

fn digraph(p: &str) -> &str {
    match p {
        "<:" => "[",
        ":>" => "]",
        "<%" => "{",
        "%>" => "}",
        "%:" => "#",
        "%:%:" => "##",
        other => other,
    }
}

/// Source text with line splices removed, plus a map back to file offsets.
struct spliced {
    bytes: Vec<u8>,
    offs: Option<Vec<u32>>,
}

impl spliced {
    fn new(text: &str) -> spliced {
        let raw = text.as_bytes();
        if !text.contains("\\\n") && !text.contains("\\\r\n") {
            return spliced { bytes: raw.to_vec(), offs: None };
        }
        let mut bytes = Vec::with_capacity(raw.len());
        let mut offs = Vec::with_capacity(raw.len());
        let mut i = 0;
        while i < raw.len() {
            if raw[i] == b'\\' && raw.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            if raw[i] == b'\\' && raw.get(i + 1) == Some(&b'\r') && raw.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            bytes.push(raw[i]);
            offs.push(i as u32);
            i += 1;
        }
        offs.push(raw.len() as u32);
        spliced { bytes, offs: Some(offs) }
    }

    fn off(&self, i: usize) -> u32 {
        match &self.offs {
            Some(o) => o[i.min(o.len() - 1)],
            None => i as u32,
        }
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 || c == b'$'
}

fn is_ident_cont(c: u8) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

/// Lexes one file into preprocessing tokens. Keywords are left as
/// identifiers; [`classify`] marks them after preprocessing.
pub fn lex_file(file_id: u32) -> Vec<token> {
    let f = diag::file(file_id);
    let crust = f.kind.is_crust();
    let src = spliced::new(&f.text);
    let b = &src.bytes;
    let mut toks = Vec::new();
    let mut i = 0;
    let mut flags = FlagBol;
    let base_flags = if crust { FlagCrust } else { 0 };
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            flags = FlagBol;
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' || c == 0x0b || c == 0x0c {
            flags |= FlagSpace;
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            flags |= FlagSpace;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= b.len() {
                let sp = span { file: file_id, lo: src.off(start), hi: src.off(start + 2) };
                diag::error("CR1101", sp, "unterminated comment");
                i = b.len();
            } else {
                i += 2;
            }
            flags |= FlagSpace;
            continue;
        }
        let start = i;
        let kind;
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            i += 1;
            while i < b.len() {
                let d = b[i];
                if (d == b'+' || d == b'-') && matches!(b[i - 1], b'e' | b'E' | b'p' | b'P') {
                    i += 1;
                } else if d == b'\'' && b.get(i + 1).is_some_and(|n| n.is_ascii_alphanumeric()) {
                    i += 1;
                } else if d.is_ascii_alphanumeric() || d == b'_' || d == b'.' {
                    i += 1;
                } else {
                    break;
                }
            }
            kind = TokenKindNumber;
        } else if let Some(q) = quote_start(b, i) {
            i = scan_quoted(b, q, file_id, &src);
            kind = if b[q] == b'"' { TokenKindString } else { TokenKindChar };
        } else if c == b'\'' && crust && b.get(i + 1).is_some_and(|&d| is_ident_start(d)) && {
            let mut j = i + 1;
            while j < b.len() && is_ident_cont(b[j]) {
                j += 1;
            }
            b.get(j) != Some(&b'\'')
        } {
            i += 1;
            while i < b.len() && is_ident_cont(b[i]) {
                i += 1;
            }
            kind = TokenKindLifetime;
        } else if is_ident_start(c) {
            while i < b.len() && is_ident_cont(b[i]) {
                i += 1;
            }
            kind = TokenKindIdent;
        } else if let Some(p) = Puncts.iter().find(|p| b[i..].starts_with(p.as_bytes())) {
            i += p.len();
            let text = sym::intern(digraph(p));
            let sp = span { file: file_id, lo: src.off(start), hi: src.off(i) };
            toks.push(token {
                kind: TokenKindPunct,
                text,
                span: sp,
                flags: flags | base_flags,
                hideset: None,
            });
            flags = 0;
            continue;
        } else {
            // A stray character. It is an error only if it survives preprocessing.
            i += 1;
            while i < b.len() && b[i] >= 0x80 && b[i] < 0xc0 {
                i += 1;
            }
            kind = TokenKindOther;
        }
        let text = String::from_utf8_lossy(&b[start..i]);
        let sp = span { file: file_id, lo: src.off(start), hi: src.off(i) };
        toks.push(token {
            kind,
            text: sym::intern(&text),
            span: sp,
            flags: flags | base_flags,
            hideset: None,
        });
        flags = 0;
    }
    let end = f.text.len() as u32;
    toks.push(token::eof(span { file: file_id, lo: end, hi: end }));
    toks
}

/// If a character or string literal (with an optional encoding prefix) starts
/// at `i`, returns the index of its opening quote.
fn quote_start(b: &[u8], i: usize) -> Option<usize> {
    let rest = &b[i..];
    for prefix in [&b"u8"[..], b"u", b"U", b"L", b""] {
        if rest.starts_with(prefix) {
            let q = i + prefix.len();
            if q < b.len() && (b[q] == b'"' || b[q] == b'\'') {
                return Some(q);
            }
        }
    }
    None
}

fn scan_quoted(b: &[u8], q: usize, file_id: u32, src: &spliced) -> usize {
    let close = b[q];
    let mut i = q + 1;
    while i < b.len() && b[i] != close {
        if b[i] == b'\n' {
            break;
        }
        if b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    if i >= b.len() || b[i] != close {
        let sp = span { file: file_id, lo: src.off(q), hi: src.off(q + 1) };
        let what = if close == b'"' { "string literal" } else { "character constant" };
        diag::error("CR1102", sp, format!("unterminated {what}"));
        return i.min(b.len());
    }
    i + 1
}

/// Keywords of C23, plus the GNU spellings that system headers use, mapped to
/// their canonical spelling.
fn c_keyword(s: sym) -> Option<sym> {
    let k = match s {
        SymAlignas | SymAlignof | SymAuto | SymBool | SymBreak | SymCase | SymChar | SymConst
        | SymConstexpr | SymContinue | SymDefault | SymDo | SymDouble | SymElse | SymEnum
        | SymExtern | SymFalse | SymFloat | SymFor | SymGoto | SymIf | SymInline | SymInt
        | SymLong | SymNullptr | SymRegister | SymRestrict | SymReturn | SymShort | SymSigned
        | SymSizeof | SymStatic | SymStaticAssert | SymStruct | SymSwitch | SymThreadLocal
        | SymTrue | SymTypedef | SymTypeof | SymTypeofUnqual | SymUnion | SymUnsigned | SymVoid
        | SymVolatile | SymWhile | SymUAtomic | SymUBitInt | SymUComplex | SymUDecimal32
        | SymUDecimal64 | SymUDecimal128 | SymUGeneric | SymUImaginary | SymUNoreturn
        | SymGnuAttribute | SymGnuExtension | SymInt128 | SymBuiltinVaList | SymBuiltinVaArg
        | SymBuiltinOffsetof => s,
        SymUAlignas => SymAlignas,
        SymUAlignof | SymGnuAlignof => SymAlignof,
        SymUBool => SymBool,
        SymUStaticAssert => SymStaticAssert,
        SymUThreadLocal => SymThreadLocal,
        SymGnuAttribute2 => SymGnuAttribute,
        SymGnuAsm | SymGnuAsm2 => SymGnuAsm,
        SymGnuRestrict | SymGnuRestrict2 => SymRestrict,
        SymGnuInline | SymGnuInline2 => SymInline,
        SymGnuConst => SymConst,
        SymGnuSigned => SymSigned,
        SymGnuVolatile => SymVolatile,
        SymGnuTypeof | SymGnuTypeof2 => SymTypeof,
        _ => return None,
    };
    Some(k)
}

fn crust_keyword(s: sym) -> bool {
    matches!(s, SymLet | SymMut | SymUnsafe | SymModule | SymExport | SymImport | SymRef)
}

/// Marks keywords after preprocessing. Crust keywords are reserved only in
/// tokens spelled in Crust files (docs/MODULES.md, section 8).
pub fn classify(toks: &mut [token]) {
    for t in toks.iter_mut() {
        if t.kind != TokenKindIdent {
            continue;
        }
        if let Some(k) = c_keyword(t.text) {
            t.kind = TokenKindKeyword;
            t.text = k;
        } else if t.has(FlagCrust) && crust_keyword(t.text) {
            t.kind = TokenKindKeyword;
        }
    }
}

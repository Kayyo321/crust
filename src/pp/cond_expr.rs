/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Evaluates `#if` expressions.
use super::macro_def;
use crate::diag;
use crate::lex::*;
use std::collections::HashMap;
use std::rc::Rc;

/// A value of type intmax_t or uintmax_t.
#[derive(Copy, Clone)]
struct pp_val {
    v: i128,
    unsigned: bool,
}

struct evaluator<'a> {
    toks: &'a [token],
    pos: usize,
    failed: bool,
}

pub fn eval(toks: &[token], _macros: &HashMap<sym, Rc<macro_def>>) -> i64 {
    let mut e = evaluator { toks, pos: 0, failed: false };
    let v = e.cond();
    if e.pos < toks.len() && !e.failed {
        diag::error(
            "CR1002",
            toks[e.pos].span,
            format!("unexpected `{}` in #if expression", toks[e.pos].text),
        );
        return 0;
    }
    if e.failed {
        return 0;
    }
    v.v as i64
}

fn wrap(v: i128, unsigned: bool) -> pp_val {
    if unsigned {
        pp_val { v: (v as u64) as i128, unsigned }
    } else {
        pp_val { v: (v as i64) as i128, unsigned }
    }
}

impl evaluator<'_> {
    fn peek(&self) -> Option<&token> {
        self.toks.get(self.pos)
    }

    fn at(&self, s: sym) -> bool {
        self.peek().is_some_and(|t| t.kind == TokenKindPunct && t.text == s)
    }

    fn fail(&mut self, msg: &str) -> pp_val {
        if !self.failed {
            let sp = self.peek().or(self.toks.last()).map(|t| t.span).unwrap_or_default();
            diag::error("CR1002", sp, format!("invalid #if expression: {msg}"));
        }
        self.failed = true;
        pp_val { v: 0, unsigned: false }
    }

    fn cond(&mut self) -> pp_val {
        let c = self.binary(0);
        if self.at(SymQuestion) {
            self.pos += 1;
            let a = self.cond();
            if !self.at(SymColon) {
                return self.fail("expected `:`");
            }
            self.pos += 1;
            let b = self.cond();
            let unsigned = a.unsigned || b.unsigned;
            return wrap(if c.v != 0 { a.v } else { b.v }, unsigned);
        }
        c
    }

    fn prec(s: sym) -> Option<u8> {
        Some(match s {
            SymPipePipe => 1,
            SymAmpAmp => 2,
            SymPipe => 3,
            SymCaret => 4,
            SymAmp => 5,
            SymEqEq | SymNe => 6,
            SymLt | SymGt | SymLe | SymGe => 7,
            SymShl | SymShr => 8,
            SymPlus | SymMinus => 9,
            SymStar | SymSlash | SymPercent => 10,
            _ => return None,
        })
    }

    fn binary(&mut self, min: u8) -> pp_val {
        let mut lhs = self.unary();
        loop {
            let Some(t) = self.peek() else { break };
            if t.kind != TokenKindPunct {
                break;
            }
            let op = t.text;
            let Some(p) = Self::prec(op) else { break };
            if p <= min {
                break;
            }
            self.pos += 1;
            let rhs = self.binary(p);
            lhs = self.apply(op, lhs, rhs);
        }
        lhs
    }

    fn apply(&mut self, op: sym, a: pp_val, b: pp_val) -> pp_val {
        let u = a.unsigned || b.unsigned;
        let (x, y) = if u { ((a.v as u64) as i128, (b.v as u64) as i128) } else { (a.v, b.v) };
        let bool_v = |c: bool| pp_val { v: c as i128, unsigned: false };
        match op {
            SymPipePipe => bool_v(a.v != 0 || b.v != 0),
            SymAmpAmp => bool_v(a.v != 0 && b.v != 0),
            SymPipe => wrap(x | y, u),
            SymCaret => wrap(x ^ y, u),
            SymAmp => wrap(x & y, u),
            SymEqEq => bool_v(x == y),
            SymNe => bool_v(x != y),
            SymLt => bool_v(x < y),
            SymGt => bool_v(x > y),
            SymLe => bool_v(x <= y),
            SymGe => bool_v(x >= y),
            SymShl => wrap(x.wrapping_shl(y as u32 & 63), a.unsigned),
            SymShr => wrap(x >> (y as u32 & 63), a.unsigned),
            SymPlus => wrap(x.wrapping_add(y), u),
            SymMinus => wrap(x.wrapping_sub(y), u),
            SymStar => wrap(x.wrapping_mul(y), u),
            SymSlash | SymPercent => {
                if y == 0 {
                    return self.fail("division by zero");
                }
                wrap(if op == SymSlash { x / y } else { x % y }, u)
            }
            _ => self.fail("unknown operator"),
        }
    }

    fn unary(&mut self) -> pp_val {
        let Some(t) = self.peek().cloned() else {
            return self.fail("expected a value");
        };
        self.pos += 1;
        match t.kind {
            TokenKindPunct => match t.text {
                SymMinus => {
                    let v = self.unary();
                    wrap(v.v.wrapping_neg(), v.unsigned)
                }
                SymPlus => self.unary(),
                SymBang => {
                    let v = self.unary();
                    pp_val { v: (v.v == 0) as i128, unsigned: false }
                }
                SymTilde => {
                    let v = self.unary();
                    wrap(!v.v, v.unsigned)
                }
                SymLParen => {
                    let v = self.cond();
                    if !self.at(SymRParen) {
                        return self.fail("expected `)`");
                    }
                    self.pos += 1;
                    v
                }
                _ => {
                    self.pos -= 1;
                    self.fail("expected a value")
                }
            },
            TokenKindNumber => match parse_pp_int(t.text.as_str()) {
                Some((v, u)) => wrap(v as i128, u),
                None => self.fail("not an integer constant"),
            },
            TokenKindChar => {
                let v = crate::parse::literal::char_value(t.text.as_str()).unwrap_or(0);
                pp_val { v: v as i128, unsigned: false }
            }
            TokenKindIdent | TokenKindKeyword => {
                // C23: true and false are 1 and 0; other identifiers are 0.
                let v = if t.text == SymTrue { 1 } else { 0 };
                pp_val { v, unsigned: false }
            }
            _ => {
                self.pos -= 1;
                self.fail("expected a value")
            }
        }
    }
}

/// Parses an integer constant as `#if` sees it. Returns the value and whether
/// it is unsigned.
pub fn parse_pp_int(s: &str) -> Option<(u64, bool)> {
    let s: String = s.chars().filter(|c| *c != '\'').collect();
    let lower = s.to_ascii_lowercase();
    let digits_end = lower.trim_end_matches(['u', 'l', 'z']).len();
    let (body, suffix) = lower.split_at(digits_end);
    let unsigned = suffix.contains('u');
    let v = if let Some(h) = body.strip_prefix("0x") {
        u64::from_str_radix(h, 16).ok()?
    } else if let Some(b) = body.strip_prefix("0b") {
        u64::from_str_radix(b, 2).ok()?
    } else if body.len() > 1 && body.starts_with('0') {
        u64::from_str_radix(&body[1..], 8).ok()?
    } else {
        body.parse::<u64>().ok()?
    };
    Some((v, unsigned || v > i64::MAX as u64))
}

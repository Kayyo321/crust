/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::sema;
use super::tir::*;
use super::types::*;
use crate::diag;
use crate::parse::ast::*;

/// Truncates or extends an integer value to the width and signedness of `t`.
pub fn wrap_to(v: i128, t: type_id) -> i128 {
    if matches!(kind(t), TypeKindBool) {
        return (v != 0) as i128;
    }
    let Some(k) = int_kind(t) else { return v };
    let bits = int_size(k) * 8;
    if bits >= 128 {
        return v;
    }
    let mask = (1i128 << bits) - 1;
    let u = v & mask;
    if int_signed(k) && u >> (bits - 1) & 1 == 1 { u - (1i128 << bits) } else { u }
}

/// Evaluates a typed expression as a constant, if it is one.
pub fn eval(e: &texpr) -> Option<ConstValue> {
    match &e.kind {
        TExprInt(v) => Some(ConstValueInt(*v)),
        TExprFloat(f) => Some(ConstValueFloat(*f)),
        TExprCast(k, inner) => {
            let v = eval(inner)?;
            let to = e.ty.ty;
            match (k, v) {
                (CastKindNoop, v) => Some(v),
                (CastKindIntToInt, ConstValueInt(i)) => Some(ConstValueInt(wrap_to(i, to))),
                (CastKindToBool, ConstValueInt(i)) => Some(ConstValueInt((i != 0) as i128)),
                (CastKindToBool, ConstValueFloat(f)) => Some(ConstValueInt((f != 0.0) as i128)),
                (CastKindIntToFloat, ConstValueInt(i)) => {
                    let f = if is_signed(inner.ty.ty) { i as f64 } else { (i as u128) as f64 };
                    Some(ConstValueFloat(if matches!(kind(to), TypeKindFloat(FloatKindFloat)) {
                        f as f32 as f64
                    } else {
                        f
                    }))
                }
                (CastKindFloatToInt, ConstValueFloat(f)) => {
                    Some(ConstValueInt(wrap_to(f as i128, to)))
                }
                (CastKindFloatToFloat, ConstValueFloat(f)) => {
                    Some(ConstValueFloat(if matches!(kind(to), TypeKindFloat(FloatKindFloat)) {
                        f as f32 as f64
                    } else {
                        f
                    }))
                }
                (CastKindNullToPtr | CastKindIntToPtr, ConstValueInt(i)) => Some(ConstValueInt(i)),
                _ => None,
            }
        }
        TExprUnary(op, a) => {
            let v = eval(a)?;
            let t = e.ty.ty;
            match (op, v) {
                (UnOpPlus, v) => Some(v),
                (UnOpNeg, ConstValueInt(i)) => Some(ConstValueInt(wrap_to(i.wrapping_neg(), t))),
                (UnOpNeg, ConstValueFloat(f)) => Some(ConstValueFloat(-f)),
                (UnOpBitNot, ConstValueInt(i)) => Some(ConstValueInt(wrap_to(!i, t))),
                (UnOpNot, ConstValueInt(i)) => Some(ConstValueInt((i == 0) as i128)),
                (UnOpNot, ConstValueFloat(f)) => Some(ConstValueInt((f == 0.0) as i128)),
                _ => None,
            }
        }
        TExprLogical(and, a, b) => {
            let x = truthy(eval(a)?);
            if *and && !x {
                return Some(ConstValueInt(0));
            }
            if !*and && x {
                return Some(ConstValueInt(1));
            }
            Some(ConstValueInt(truthy(eval(b)?) as i128))
        }
        TExprCond(c, a, b) => {
            if truthy(eval(c)?) {
                eval(a)
            } else {
                eval(b)
            }
        }
        TExprComma(..) => None,
        TExprBinary(op, a, b) => {
            let x = eval(a)?;
            let y = eval(b)?;
            binary(*op, x, y, a.ty.ty, e.ty.ty)
        }
        _ => None,
    }
}

fn truthy(v: ConstValue) -> bool {
    match v {
        ConstValueInt(i) => i != 0,
        ConstValueFloat(f) => f != 0.0,
    }
}

fn binary(
    op: BinOp,
    x: ConstValue,
    y: ConstValue,
    operand_ty: type_id,
    result_ty: type_id,
) -> Option<ConstValue> {
    match (x, y) {
        (ConstValueInt(a), ConstValueInt(b)) => {
            let unsigned = !is_signed(operand_ty);
            let (ua, ub) = (a as u128, b as u128);
            let r = match op {
                BinOpAdd => a.wrapping_add(b),
                BinOpSub => a.wrapping_sub(b),
                BinOpMul => a.wrapping_mul(b),
                BinOpDiv | BinOpRem => {
                    if b == 0 {
                        return None;
                    }
                    match (op, unsigned) {
                        (BinOpDiv, true) => wrap_to(a, operand_ty) as u128 as i128 / b,
                        (BinOpDiv, false) => a.wrapping_div(b),
                        (_, true) => (wrap_to(a, operand_ty) as u128 % ub) as i128,
                        (_, false) => a.wrapping_rem(b),
                    }
                }
                BinOpShl => a.wrapping_shl(b as u32),
                BinOpShr => {
                    if unsigned {
                        let bits = size_of(operand_ty) * 8;
                        let mask = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
                        ((ua & mask) >> (b as u32)) as i128
                    } else {
                        a >> (b as u32).min(127)
                    }
                }
                BinOpBitAnd => a & b,
                BinOpBitOr => a | b,
                BinOpBitXor => a ^ b,
                BinOpLt | BinOpGt | BinOpLe | BinOpGe | BinOpEq | BinOpNe => {
                    let (a2, b2) = if unsigned {
                        (
                            wrap_to(a, to_unsigned_ty(operand_ty)) as u128 as i128,
                            wrap_to(b, to_unsigned_ty(operand_ty)) as u128 as i128,
                        )
                    } else {
                        (a, b)
                    };
                    let c = match op {
                        BinOpLt => a2 < b2,
                        BinOpGt => a2 > b2,
                        BinOpLe => a2 <= b2,
                        BinOpGe => a2 >= b2,
                        BinOpEq => a2 == b2,
                        _ => a2 != b2,
                    };
                    return Some(ConstValueInt(c as i128));
                }
                BinOpLogAnd => ((a != 0) && (b != 0)) as i128,
                BinOpLogOr => ((a != 0) || (b != 0)) as i128,
            };
            Some(ConstValueInt(wrap_to(r, result_ty)))
        }
        (ConstValueFloat(a), ConstValueFloat(b)) => {
            let r = match op {
                BinOpAdd => a + b,
                BinOpSub => a - b,
                BinOpMul => a * b,
                BinOpDiv => a / b,
                BinOpLt => return Some(ConstValueInt((a < b) as i128)),
                BinOpGt => return Some(ConstValueInt((a > b) as i128)),
                BinOpLe => return Some(ConstValueInt((a <= b) as i128)),
                BinOpGe => return Some(ConstValueInt((a >= b) as i128)),
                BinOpEq => return Some(ConstValueInt((a == b) as i128)),
                BinOpNe => return Some(ConstValueInt((a != b) as i128)),
                _ => return None,
            };
            Some(ConstValueFloat(r))
        }
        _ => None,
    }
}

fn to_unsigned_ty(t: type_id) -> type_id {
    match int_kind(t) {
        Some(k) => int_of(to_unsigned(k)),
        None => t,
    }
}

impl sema<'_> {
    /// Evaluates an integer constant expression, reporting CR1308 if it is not one.
    pub fn const_int_expr(&mut self, e: &expr) -> Option<i128> {
        let r = self.const_int_expr_quiet(e);
        if r.is_none() {
            diag::error("CR1308", e.span, "expected an integer constant expression");
        }
        r
    }

    pub fn const_int_expr_quiet(&mut self, e: &expr) -> Option<i128> {
        self.unevaluated += 1;
        let te = self.expr(e);
        let te = self.to_rvalue(te);
        self.unevaluated -= 1;
        if te.is_error() {
            return Some(0);
        }
        if !is_integer(te.ty.ty) {
            return None;
        }
        match eval(&te) {
            Some(ConstValueInt(v)) => Some(v),
            _ => None,
        }
    }
}

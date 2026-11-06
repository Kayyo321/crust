/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::consteval;
use super::tir::*;
use super::types::*;
use super::*;
use crate::diag;
use crate::parse::literal;

fn bx(e: texpr) -> Box<texpr> {
    Box::new(e)
}

impl sema<'_> {
    /// Checks an expression. Lvalues are returned as lvalues (no load).
    pub fn expr(&mut self, e: &expr) -> texpr {
        let sp = e.span;
        match &e.kind {
            ExprKindIdent(name) => self.ident(*name, sp),
            ExprKindQualified(ns, name) => self.qualified(*ns, *name, sp),
            ExprKindInt(text) => self.int_literal(*text, sp),
            ExprKindFloat(text) => match literal::float_literal(text.as_str()) {
                Ok((v, k)) => {
                    let fk = match k {
                        1 => FloatKindFloat,
                        2 => FloatKindLongDouble,
                        _ => FloatKindDouble,
                    };
                    let v = if fk == FloatKindFloat { v as f32 as f64 } else { v };
                    texpr::rvalue(TExprFloat(v), q(float_of(fk)), sp)
                }
                Err(m) => {
                    diag::error("CR1308", sp, m);
                    texpr::error(sp)
                }
            },
            ExprKindChar(text) => match literal::char_literal(text.as_str()) {
                Ok((v, enc)) => {
                    let t = match enc {
                        literal::EncodingPlain => int(),
                        literal::EncodingUtf8 => int_of(IntKindUChar),
                        literal::EncodingUtf16 => int_of(IntKindUShort),
                        literal::EncodingUtf32 => int_of(IntKindUInt),
                        literal::EncodingWide => int(),
                    };
                    texpr::int_const(v as i128, t, sp)
                }
                Err(m) => {
                    diag::error("CR1102", sp, m);
                    texpr::error(sp)
                }
            },
            ExprKindString(pieces) => {
                let strs: Vec<&str> = pieces.iter().map(|p| p.as_str()).collect();
                match literal::string_literal(&strs) {
                    Ok((units, enc)) => {
                        let id = self.intern_string(units, enc);
                        let ty = self.prog.strings[id as usize].ty;
                        texpr { kind: TExprString(id), ty: q(ty), lvalue: true, span: sp }
                    }
                    Err(m) => {
                        diag::error("CR1102", sp, m);
                        texpr::error(sp)
                    }
                }
            }
            ExprKindBool(b) => texpr::int_const(*b as i128, bool_(), sp),
            ExprKindNullptr => texpr::rvalue(TExprNull, q(intern(TypeKindNullptr)), sp),
            ExprKindUnary(op, a) => self.unary(*op, a, sp),
            ExprKindBinary(op, a, b) => self.binary(*op, a, b, sp),
            ExprKindAssign(op, a, b) => self.assign(*op, a, b, sp),
            ExprKindCond(c, a, b) => self.conditional(c, a, b, sp),
            ExprKindComma(a, b) => {
                let x = self.expr(a);
                let x = self.to_rvalue(x);
                let y = self.expr(b);
                let y = self.to_rvalue(y);
                let t = y.ty;
                texpr::rvalue(TExprComma(bx(x), bx(y)), t, sp)
            }
            ExprKindCall(f, args) => self.call(f, args, sp),
            ExprKindIndex(a, i) => self.index(a, i, sp),
            ExprKindMember { base, name, name_span, arrow } => {
                self.member(base, *name, *name_span, *arrow, sp)
            }
            ExprKindCast(tn, x) => {
                let to = self.type_name(tn);
                let v = self.expr(x);
                let v = self.to_rvalue(v);
                self.explicit_cast(v, to, sp)
            }
            ExprKindCompound(tn, init) => self.compound_literal(tn, init, sp),
            ExprKindSizeofExpr(x) => {
                self.unevaluated += 1;
                let v = self.expr(x);
                self.unevaluated -= 1;
                self.sizeof_type(v.ty, sp)
            }
            ExprKindSizeofType(tn) => {
                let t = self.type_name(tn);
                self.sizeof_type(t, sp)
            }
            ExprKindAlignofType(tn) => {
                let t = self.type_name(tn);
                texpr::int_const(align_of(t.ty) as i128, size_t(), sp)
            }
            ExprKindAlignofExpr(x) => {
                self.unevaluated += 1;
                let v = self.expr(x);
                self.unevaluated -= 1;
                texpr::int_const(align_of(v.ty.ty) as i128, size_t(), sp)
            }
            ExprKindGeneric(ctrl, assocs) => self.generic(ctrl, assocs, sp),
            ExprKindRef(mutable, x) => self.borrow(*mutable, x, sp),
            ExprKindVaArg(ap, tn) => {
                let a = self.expr(ap);
                let a = self.to_rvalue(a);
                let t = self.type_name(tn);
                texpr::rvalue(TExprVaArg(bx(a)), t.unqual(), sp)
            }
            ExprKindOffsetof(tn, path) => self.offsetof(tn, path, sp),
        }
    }

    pub fn rvalue_expr(&mut self, e: &expr) -> texpr {
        let t = self.expr(e);
        self.to_rvalue(t)
    }

    /// Lvalue conversion, array-to-pointer and function-to-pointer decay.
    pub fn to_rvalue(&mut self, e: texpr) -> texpr {
        if !e.lvalue && !is_function(e.ty.ty) {
            return e;
        }
        let sp = e.span;
        match e.ty.kind() {
            TypeKindArray(elem, _) => {
                let t = q(pointer_to(elem));
                texpr::rvalue(TExprCast(CastKindArrayDecay, bx(e)), t, sp)
            }
            TypeKindFunction(_) => {
                let t = q(pointer_to(e.ty));
                texpr::rvalue(TExprCast(CastKindFuncDecay, bx(e)), t, sp)
            }
            _ => {
                if self.reads_union_member(&e) {
                    self.require_unsafe(sp, "reading a union member");
                }
                if let TExprLocal(l) = e.kind {
                    if self.deferred_lets.contains(&l) {
                        let n = self.fctx.as_ref().unwrap().locals[l as usize].name;
                        diag::error("CR1322", sp, format!("the type of `{n}` is not known yet"))
                            .help("assign it before using it, or write its type: `let name: T;`");
                    }
                }
                let t = e.ty.unqual();
                texpr::rvalue(TExprLoad(bx(e)), t, sp)
            }
        }
    }

    pub fn decay(&mut self, e: texpr) -> texpr {
        self.to_rvalue(e)
    }

    fn reads_union_member(&self, e: &texpr) -> bool {
        match &e.kind {
            TExprMember(base, rec, _) => record(*rec).is_union || self.reads_union_member(base),
            _ => false,
        }
    }

    fn ident(&mut self, name: sym, sp: span) -> texpr {
        if name == SymFunc {
            let fname = self.fctx.as_ref().map(|f| f.name.to_string()).unwrap_or_default();
            let units: Vec<u32> = fname.bytes().map(|b| b as u32).collect();
            let id = self.intern_string(units, literal::EncodingPlain);
            let ty = self.prog.strings[id as usize].ty;
            return texpr {
                kind: TExprString(id),
                ty: q(ty).with(QualConst),
                lvalue: true,
                span: sp,
            };
        }
        match self.lookup(name) {
            Some(BindingLocal(l)) => {
                let ty = self.fctx.as_ref().unwrap().locals[l as usize].ty;
                texpr { kind: TExprLocal(l), ty, lvalue: true, span: sp }
            }
            Some(BindingGlobal(g)) => self.global_ref(g, sp),
            Some(BindingConst(v, t)) => match v {
                ConstValueInt(i) => texpr::rvalue(TExprInt(i), t, sp),
                ConstValueFloat(f) => texpr::rvalue(TExprFloat(f), t, sp),
            },
            Some(BindingTypedef(_)) => {
                diag::error("CR1315", sp, format!("`{name}` is a type, not a value"));
                texpr::error(sp)
            }
            None => {
                let mut d =
                    diag::error("CR1302", sp, format!("use of undeclared identifier `{name}`"));
                if let Some(h) = std_header_hint(name) {
                    d = d.help(format!("`{name}` is declared in `<{h}>`; add `#include <{h}>`"));
                }
                drop(d);
                texpr::error(sp)
            }
        }
    }

    pub fn global_ref(&mut self, g: global_id, sp: span) -> texpr {
        let gd = &mut self.prog.globals[g as usize];
        gd.used = true;
        let (ty, needs_unsafe, name) = (gd.ty, gd.needs_unsafe, gd.name);
        if needs_unsafe {
            self.require_unsafe(sp, &format!("use of the mutable module-level variable `{name}`"));
        }
        let lvalue = !is_function(ty.ty);
        texpr { kind: TExprGlobal(g), ty, lvalue, span: sp }
    }

    fn qualified(&mut self, ns: sym, name: sym, sp: span) -> texpr {
        let Some(iface) = self.namespaces.get(&ns).cloned() else {
            diag::error("CR1302", sp, format!("`{ns}` is not a namespace import"));
            return texpr::error(sp);
        };
        let items: Vec<export_item> =
            iface.find(name).filter(|e| !e.kind.is_tag()).cloned().collect();
        let Some(e) = items.into_iter().next() else {
            diag::error("CR1405", sp, format!("module `{}` does not export `{name}`", iface.name));
            return texpr::error(sp);
        };
        match &e.kind {
            ExportKindConst { ty, value } => match value {
                ConstValueInt(i) => texpr::rvalue(TExprInt(*i), *ty, sp),
                ConstValueFloat(f) => texpr::rvalue(TExprFloat(*f), *ty, sp),
            },
            ExportKindFunc { .. } | ExportKindVar { .. } => {
                let g = self.import_global(&e, iface.name, sp);
                self.global_ref(g, sp)
            }
            _ => {
                diag::error("CR1315", sp, format!("`{ns}::{name}` is a type, not a value"));
                texpr::error(sp)
            }
        }
    }

    fn int_literal(&mut self, text: sym, sp: span) -> texpr {
        let (v, sf) = match literal::int_literal(text.as_str()) {
            Ok(x) => x,
            Err(m) => {
                diag::error("CR1308", sp, m);
                return texpr::error(sp);
            }
        };
        let candidates: &[IntKind] = match (sf.unsigned, sf.long, sf.decimal) {
            (false, 0, true) => &[IntKindInt, IntKindLong, IntKindLongLong],
            (false, 0, false) => &[
                IntKindInt,
                IntKindUInt,
                IntKindLong,
                IntKindULong,
                IntKindLongLong,
                IntKindULongLong,
            ],
            (true, 0, _) => &[IntKindUInt, IntKindULong, IntKindULongLong],
            (false, 1, true) => &[IntKindLong, IntKindLongLong],
            (false, 1, false) => &[IntKindLong, IntKindULong, IntKindLongLong, IntKindULongLong],
            (true, 1, _) => &[IntKindULong, IntKindULongLong],
            (false, _, true) => &[IntKindLongLong],
            (false, _, false) => &[IntKindLongLong, IntKindULongLong],
            (true, _, _) => &[IntKindULongLong],
        };
        for k in candidates {
            let bits = int_size(*k) * 8;
            let max: u128 =
                if int_signed(*k) { (1u128 << (bits - 1)) - 1 } else { (1u128 << bits) - 1 };
            if v <= max {
                return texpr::int_const(v as i128, int_of(*k), sp);
            }
        }
        diag::error(
            "CR1308",
            sp,
            format!("integer constant `{text}` is too large for any integer type"),
        );
        texpr::error(sp)
    }

    fn sizeof_type(&mut self, t: qual_type, sp: span) -> texpr {
        if is_function(t.ty) || !is_complete(t.ty) {
            if !is_error(t.ty) {
                diag::error(
                    "CR1310",
                    sp,
                    format!("`sizeof` applied to incomplete type `{}`", display(t)),
                );
            }
            return texpr::int_const(1, size_t(), sp);
        }
        texpr::int_const(size_of(t.ty) as i128, size_t(), sp)
    }

    fn is_null_constant(&self, e: &texpr) -> bool {
        if matches!(e.ty.kind(), TypeKindNullptr) {
            return true;
        }
        if is_integer(e.ty.ty) {
            return matches!(consteval::eval(e), Some(ConstValueInt(0)));
        }
        if let (TExprCast(_, inner), TypeKindPointer(p)) = (&e.kind, e.ty.kind()) {
            return is_void(p.ty) && p.quals == 0 && self.is_null_constant(inner);
        }
        false
    }

    // ---- conversions ----

    /// Inserts a cast of kind chosen from the two types. No checking.
    pub fn cast_to(&self, e: texpr, to: qual_type) -> texpr {
        let from = e.ty.ty;
        let sp = e.span;
        if from == to.ty || is_error(from) || is_error(to.ty) {
            if from == to.ty && e.ty.quals == to.quals {
                return e;
            }
            return texpr::rvalue(TExprCast(CastKindNoop, bx(e)), to.unqual(), sp);
        }
        let k = match (kind(from), kind(to.ty)) {
            (_, TypeKindVoid) => CastKindToVoid,
            (_, TypeKindBool) => CastKindToBool,
            (TypeKindFloat(_), TypeKindFloat(_)) => CastKindFloatToFloat,
            (TypeKindFloat(_), _) if is_integer(to.ty) => CastKindFloatToInt,
            (_, TypeKindFloat(_)) if is_integer(from) => CastKindIntToFloat,
            (TypeKindNullptr, TypeKindPointer(_)) => CastKindNullToPtr,
            (_, TypeKindPointer(_)) if is_integer(from) => {
                if self.is_null_constant(&e) {
                    CastKindNullToPtr
                } else {
                    CastKindIntToPtr
                }
            }
            (TypeKindPointer(_), _) if is_integer(to.ty) => CastKindPtrToInt,
            (TypeKindPointer(_), TypeKindPointer(_)) => CastKindPtrToPtr,
            (TypeKindRef { .. }, TypeKindPointer(_)) => CastKindRefToPtr,
            (TypeKindRef { .. }, TypeKindRef { .. }) => CastKindRefWeaken,
            (TypeKindRef { .. }, TypeKindSlice { .. }) => CastKindArrayRefToSlice,
            (TypeKindSlice { .. }, TypeKindSlice { .. }) => CastKindSliceWeaken,
            _ if is_integer(from) && is_integer(to.ty) => CastKindIntToInt,
            _ => CastKindNoop,
        };
        texpr::rvalue(TExprCast(k, bx(e)), to.unqual(), sp)
    }

    /// Converts as if by assignment (C23 6.5.17.2), reporting mismatches.
    pub fn convert_assign(&mut self, e: texpr, to: qual_type, sp: span, what: &str) -> texpr {
        let e = self.to_rvalue(e);
        let from = e.ty.ty;
        let t = to.ty;
        if is_error(from) || is_error(t) {
            return self.cast_to(e, to);
        }
        if compatible_unqual(from, t) {
            if let (TypeKindRef { .. }, TypeKindRef { .. }) = (kind(from), kind(t)) {
                return self.cast_to(e, to);
            }
            return if from == t { e } else { self.cast_to(e, to) };
        }
        let crust = self.crust();
        let fail = |s: &Self, e: texpr| -> texpr {
            diag::error(
                "CR1304",
                sp,
                format!("{what}: expected `{}`, found `{}`", display(to), display(e.ty)),
            )
            .label(format!("has type `{}`", display(e.ty)));
            let _ = s;
            texpr::error(e.span)
        };
        match (kind(from), kind(t)) {
            _ if is_arith(from) && is_arith(t) => self.cast_to(e, to),
            (TypeKindPointer(_) | TypeKindNullptr, TypeKindBool) => self.cast_to(e, to),
            (TypeKindPointer(fp), TypeKindPointer(tp)) => {
                let lost = fp.quals & !tp.quals;
                let ok = compatible(fp.unqual(), tp.unqual()) || is_void(fp.ty) || is_void(tp.ty);
                if !ok {
                    if crust {
                        diag::error(
                            "CR1304",
                            sp,
                            format!(
                                "{what}: incompatible pointer types: expected `{}`, found `{}`",
                                display(to),
                                display(e.ty)
                            ),
                        )
                        .help("use an explicit cast inside `unsafe` if the conversion is intended");
                    } else {
                        diag::warning(
                            "CR1304",
                            sp,
                            format!(
                                "{what}: incompatible pointer types: expected `{}`, found `{}`",
                                display(to),
                                display(e.ty)
                            ),
                        );
                    }
                } else if lost != 0 && !is_void(tp.ty) || lost != 0 && is_void(tp.ty) {
                    let m = format!(
                        "{what}: conversion from `{}` to `{}` discards qualifiers",
                        display(e.ty),
                        display(to)
                    );
                    if crust {
                        diag::error("CR1323", sp, m);
                    } else {
                        diag::warning("CR1323", sp, m);
                    }
                }
                self.cast_to(e, to)
            }
            (_, TypeKindPointer(_)) if self.is_null_constant(&e) => self.cast_to(e, to),
            (_, TypeKindPointer(_)) if is_integer(from) => {
                let m = format!("{what}: integer to pointer conversion without a cast");
                if crust {
                    diag::error("CR1304", sp, m);
                } else {
                    diag::warning("CR1304", sp, m);
                }
                self.cast_to(e, to)
            }
            (TypeKindPointer(_), _) if is_integer(t) => {
                let m = format!("{what}: pointer to integer conversion without a cast");
                if crust {
                    diag::error("CR1304", sp, m);
                } else {
                    diag::warning("CR1304", sp, m);
                }
                self.cast_to(e, to)
            }
            (
                TypeKindRef { mutable: fm, pointee: fp, .. },
                TypeKindRef { mutable: tm, pointee: tp, .. },
            ) => {
                if (tm && !fm) || !compatible(fp.unqual(), tp.unqual()) {
                    return fail(self, e);
                }
                self.cast_to(e, to)
            }
            (
                TypeKindRef { mutable: fm, pointee: fp, .. },
                TypeKindSlice { mutable: tm, elem, .. },
            ) => match fp.kind() {
                TypeKindArray(ae, Some(_))
                    if compatible(ae.unqual(), elem.unqual()) && (fm || !tm) =>
                {
                    self.cast_to(e, to)
                }
                _ => fail(self, e),
            },
            (
                TypeKindSlice { mutable: fm, elem: fe, .. },
                TypeKindSlice { mutable: tm, elem: te, .. },
            ) => {
                if (tm && !fm) || !compatible(fe.unqual(), te.unqual()) {
                    return fail(self, e);
                }
                self.cast_to(e, to)
            }
            (TypeKindRef { .. }, TypeKindPointer(_)) => {
                diag::error(
                    "CR1304",
                    sp,
                    format!(
                        "{what}: expected `{}`, found the borrow `{}`",
                        display(to),
                        display(e.ty)
                    ),
                )
                .help("convert a borrow to a raw pointer with an explicit cast");
                texpr::error(e.span)
            }
            _ => fail(self, e),
        }
    }

    fn explicit_cast(&mut self, v: texpr, to: qual_type, sp: span) -> texpr {
        let from = v.ty.ty;
        let t = to.ty;
        if is_error(from) || is_error(t) {
            return texpr::error(sp);
        }
        if is_void(t) {
            let mut r = self.cast_to(v, q(void()));
            r.span = sp;
            return r;
        }
        let ok = match (kind(from), kind(t)) {
            _ if is_scalar(from) && is_scalar(t) => true,
            (TypeKindRef { mutable, pointee, .. }, TypeKindPointer(p)) => {
                if !mutable && !p.is_const() {
                    self.require_unsafe(sp, "casting a shared borrow to a mutable pointer");
                }
                if !compatible(pointee.unqual(), p.unqual()) && !is_void(p.ty) {
                    self.require_unsafe(sp, "a cast between unrelated pointer types");
                }
                true
            }
            _ if compatible_unqual(from, t) => true,
            _ => false,
        };
        if !ok {
            diag::error(
                "CR1318",
                sp,
                format!("cannot cast `{}` to `{}`", display(v.ty), display(to)),
            );
            return texpr::error(sp);
        }
        match (kind(from), kind(t)) {
            (TypeKindPointer(fp), TypeKindPointer(tp)) => {
                let related = compatible(fp.unqual(), tp.unqual()) || is_void(tp.ty);
                if !related {
                    self.require_unsafe(sp, "a cast between unrelated pointer types");
                }
            }
            (_, TypeKindPointer(_)) if is_integer(from) && !self.is_null_constant(&v) => {
                self.require_unsafe(sp, "a cast from an integer to a pointer");
            }
            _ => {}
        }
        let mut r = self.cast_to(v, to);
        r.span = sp;
        r
    }

    fn promote_expr(&mut self, e: texpr) -> texpr {
        let p = promote(e.ty.ty);
        if p != e.ty.ty { self.cast_to(e, q(p)) } else { e }
    }

    // ---- operators ----

    pub fn check_modifiable(&mut self, e: &texpr, what: &str) -> bool {
        if e.is_error() {
            return false;
        }
        if !e.lvalue {
            diag::error("CR1306", e.span, format!("{what} needs a modifiable lvalue"));
            return false;
        }
        if is_array(e.ty.ty) || is_function(e.ty.ty) {
            diag::error(
                "CR1319",
                e.span,
                format!("{what}: arrays and functions cannot be assigned"),
            );
            return false;
        }
        if e.ty.is_const() || self.has_const_member(e.ty.ty) {
            self.report_const_target(e, what);
            return false;
        }
        true
    }

    fn has_const_member(&self, t: type_id) -> bool {
        match kind(t) {
            TypeKindRecord(r) => {
                record(r).fields.iter().any(|f| f.ty.is_const() || self.has_const_member(f.ty.ty))
            }
            _ => false,
        }
    }

    fn report_const_target(&self, e: &texpr, what: &str) {
        // Find why the target is read-only.
        let mut cur = e;
        loop {
            match &cur.kind {
                TExprLocal(l) => {
                    let li = &self.fctx.as_ref().unwrap().locals[*l as usize];
                    if li.is_let && !li.mutable {
                        diag::error(
                            "CR0384",
                            e.span,
                            format!("cannot assign twice to immutable binding `{}`", li.name),
                        )
                        .secondary(li.span, "declared here without `mut`")
                        .help(format!("make the binding mutable: `let mut {}`", li.name));
                        return;
                    }
                    break;
                }
                TExprMember(b, _, _) => cur = b,
                TExprIndex(b, _) => cur = b,
                TExprDeref(p) | TExprSliceIndex(p, _) => {
                    let pt = match &p.kind {
                        TExprLoad(x) => x.ty.ty,
                        _ => p.ty.ty,
                    };
                    if matches!(
                        kind(pt),
                        TypeKindRef { mutable: false, .. } | TypeKindSlice { mutable: false, .. }
                    ) {
                        diag::error("CR0594", e.span, format!("{what} through a shared borrow"))
                            .help("borrow with `ref mut` to mutate");
                        return;
                    }
                    break;
                }
                _ => break,
            }
        }
        diag::error("CR1306", e.span, format!("{what} to a read-only location"));
    }

    fn unary(&mut self, op: UnOp, a: &expr, sp: span) -> texpr {
        match op {
            UnOpPlus | UnOpNeg | UnOpBitNot => {
                let v = self.rvalue_expr(a);
                if v.is_error() {
                    return v;
                }
                let ok = if op == UnOpBitNot { is_integer(v.ty.ty) } else { is_arith(v.ty.ty) };
                if !ok {
                    diag::error(
                        "CR1305",
                        sp,
                        format!("invalid operand to unary operator: `{}`", display(v.ty)),
                    );
                    return texpr::error(sp);
                }
                let v = self.promote_expr(v);
                let t = v.ty;
                texpr::rvalue(TExprUnary(op, bx(v)), t, sp)
            }
            UnOpNot => {
                let v = self.rvalue_expr(a);
                if !is_scalar(v.ty.ty) && !v.is_error() {
                    diag::error(
                        "CR1305",
                        sp,
                        format!("invalid operand to `!`: `{}`", display(v.ty)),
                    );
                    return texpr::error(sp);
                }
                texpr::rvalue(TExprUnary(UnOpNot, bx(v)), q(int()), sp)
            }
            UnOpDeref => {
                let v = self.rvalue_expr(a);
                self.deref(v, sp)
            }
            UnOpAddrOf => {
                let v = self.expr(a);
                if v.is_error() {
                    return v;
                }
                if is_function(v.ty.ty) {
                    let t = q(pointer_to(v.ty));
                    return texpr::rvalue(TExprCast(CastKindFuncDecay, bx(v)), t, sp);
                }
                if !v.lvalue {
                    diag::error("CR1306", sp, "cannot take the address of an rvalue");
                    return texpr::error(sp);
                }
                if let TExprMember(_, r, i) = &v.kind {
                    if record(*r).fields[*i as usize].bits.is_some() {
                        diag::error("CR1306", sp, "cannot take the address of a bit-field");
                        return texpr::error(sp);
                    }
                }
                let t = q(pointer_to(v.ty));
                texpr::rvalue(TExprAddrOf(bx(v)), t, sp)
            }
            UnOpPreInc | UnOpPreDec | UnOpPostInc | UnOpPostDec => {
                let v = self.expr(a);
                if !self.check_modifiable(&v, "increment or decrement") {
                    return texpr::error(sp);
                }
                if is_pointer(v.ty.ty) {
                    self.require_unsafe(sp, "pointer arithmetic");
                } else if !is_arith(v.ty.ty) {
                    diag::error(
                        "CR1305",
                        sp,
                        format!("cannot increment or decrement `{}`", display(v.ty)),
                    );
                    return texpr::error(sp);
                }
                let k = match op {
                    UnOpPreInc => IncDecPreInc,
                    UnOpPreDec => IncDecPreDec,
                    UnOpPostInc => IncDecPostInc,
                    _ => IncDecPostDec,
                };
                let t = v.ty.unqual();
                texpr::rvalue(TExprIncDec(k, bx(v)), t, sp)
            }
        }
    }

    pub fn deref(&mut self, v: texpr, sp: span) -> texpr {
        if v.is_error() {
            return v;
        }
        match v.ty.kind() {
            TypeKindPointer(p) => {
                if is_void(p.ty) {
                    diag::error("CR1319", sp, "cannot dereference a `void *`");
                    return texpr::error(sp);
                }
                let lvalue = !is_function(p.ty);
                texpr { kind: TExprDeref(bx(v)), ty: p, lvalue, span: sp }
            }
            TypeKindRef { mutable, pointee, .. } => {
                let ty = if mutable { pointee } else { pointee.with(QualConst) };
                texpr { kind: TExprDeref(bx(v)), ty, lvalue: true, span: sp }
            }
            _ => {
                diag::error(
                    "CR1305",
                    sp,
                    format!("cannot dereference a value of type `{}`", display(v.ty)),
                );
                texpr::error(sp)
            }
        }
    }

    fn arith_pair(&mut self, a: texpr, b: texpr) -> (texpr, texpr, type_id) {
        let t = usual_arith(a.ty.ty, b.ty.ty);
        (self.cast_to(a, q(t)), self.cast_to(b, q(t)), t)
    }

    fn binary(&mut self, op: BinOp, a: &expr, b: &expr, sp: span) -> texpr {
        let x = self.rvalue_expr(a);
        let y = self.rvalue_expr(b);
        self.binary_values(op, x, y, sp)
    }

    pub fn binary_values(&mut self, op: BinOp, x: texpr, y: texpr, sp: span) -> texpr {
        if x.is_error() || y.is_error() {
            return texpr::error(sp);
        }
        let (tx, ty_) = (x.ty.ty, y.ty.ty);
        let bad = |x: &texpr, y: &texpr| {
            diag::error(
                "CR1305",
                sp,
                format!(
                    "invalid operands to `{}`: `{}` and `{}`",
                    op.spelling(),
                    display(x.ty),
                    display(y.ty)
                ),
            );
            texpr::error(sp)
        };
        match op {
            BinOpLogAnd | BinOpLogOr => {
                if !is_scalar(tx) || !is_scalar(ty_) {
                    return bad(&x, &y);
                }
                texpr::rvalue(TExprLogical(op == BinOpLogAnd, bx(x), bx(y)), q(int()), sp)
            }
            BinOpMul | BinOpDiv => {
                if !is_arith(tx) || !is_arith(ty_) {
                    return bad(&x, &y);
                }
                let (x, y, t) = self.arith_pair(x, y);
                texpr::rvalue(TExprBinary(op, bx(x), bx(y)), q(t), sp)
            }
            BinOpRem | BinOpBitAnd | BinOpBitOr | BinOpBitXor => {
                if !is_integer(tx) || !is_integer(ty_) {
                    return bad(&x, &y);
                }
                let (x, y, t) = self.arith_pair(x, y);
                texpr::rvalue(TExprBinary(op, bx(x), bx(y)), q(t), sp)
            }
            BinOpShl | BinOpShr => {
                if !is_integer(tx) || !is_integer(ty_) {
                    return bad(&x, &y);
                }
                let x = self.promote_expr(x);
                let y = self.promote_expr(y);
                let t = x.ty;
                texpr::rvalue(TExprBinary(op, bx(x), bx(y)), t, sp)
            }
            BinOpAdd | BinOpSub => {
                if is_arith(tx) && is_arith(ty_) {
                    let (x, y, t) = self.arith_pair(x, y);
                    return texpr::rvalue(TExprBinary(op, bx(x), bx(y)), q(t), sp);
                }
                if is_pointer(tx) && is_integer(ty_) {
                    self.check_ptr_arith(&x, sp);
                    let y = if op == BinOpSub {
                        let y = self.cast_to(y, q(ptrdiff_t()));
                        texpr::rvalue(TExprUnary(UnOpNeg, bx(y)), q(ptrdiff_t()), sp)
                    } else {
                        y
                    };
                    let t = x.ty;
                    return texpr::rvalue(TExprPtrAdd(bx(x), bx(y)), t, sp);
                }
                if op == BinOpAdd && is_integer(tx) && is_pointer(ty_) {
                    self.check_ptr_arith(&y, sp);
                    let t = y.ty;
                    return texpr::rvalue(TExprPtrAdd(bx(y), bx(x)), t, sp);
                }
                if op == BinOpSub && is_pointer(tx) && is_pointer(ty_) {
                    let (pa, pb) = (pointee(tx).unwrap(), pointee(ty_).unwrap());
                    if !compatible(pa.unqual(), pb.unqual()) {
                        return bad(&x, &y);
                    }
                    return texpr::rvalue(TExprPtrDiff(bx(x), bx(y)), q(ptrdiff_t()), sp);
                }
                bad(&x, &y)
            }
            BinOpLt | BinOpGt | BinOpLe | BinOpGe | BinOpEq | BinOpNe => {
                if is_arith(tx) && is_arith(ty_) {
                    let (x, y, _) = self.arith_pair(x, y);
                    return texpr::rvalue(TExprBinary(op, bx(x), bx(y)), q(int()), sp);
                }
                let eq = matches!(op, BinOpEq | BinOpNe);
                let ptrish = |t: type_id| is_pointer(t) || matches!(kind(t), TypeKindNullptr);
                if ptrish(tx) && ptrish(ty_) {
                    let target = if is_pointer(tx) { x.ty } else { y.ty };
                    let x = self.cast_to(x, target);
                    let y = self.cast_to(y, target);
                    return texpr::rvalue(TExprBinary(op, bx(x), bx(y)), q(int()), sp);
                }
                if eq && is_pointer(tx) && self.is_null_constant(&y) {
                    let t = x.ty;
                    let y = self.cast_to(y, t);
                    return texpr::rvalue(TExprBinary(op, bx(x), bx(y)), q(int()), sp);
                }
                if eq && is_pointer(ty_) && self.is_null_constant(&x) {
                    let t = y.ty;
                    let x = self.cast_to(x, t);
                    return texpr::rvalue(TExprBinary(op, bx(x), bx(y)), q(int()), sp);
                }
                bad(&x, &y)
            }
        }
    }

    fn check_ptr_arith(&mut self, p: &texpr, sp: span) {
        if let Some(pt) = pointee(p.ty.ty) {
            if !is_complete(pt.ty) && !is_error(pt.ty) {
                diag::error(
                    "CR1310",
                    sp,
                    format!("pointer arithmetic on a pointer to incomplete type `{}`", display(pt)),
                );
            }
        }
        self.require_unsafe(sp, "pointer arithmetic");
    }

    fn assign(&mut self, op: Option<BinOp>, a: &expr, b: &expr, sp: span) -> texpr {
        let lhs = self.expr(a);
        let rhs = self.expr(b);
        if lhs.is_error() {
            return texpr::error(sp);
        }
        let Some(op) = op else {
            // `let x;` takes its type from the first assignment.
            if let TExprLocal(l) = lhs.kind {
                if self.deferred_lets.remove(&l) {
                    let r = self.to_rvalue(rhs);
                    let fc = self.fctx.as_mut().unwrap();
                    let li = &mut fc.locals[l as usize];
                    li.ty = if li.mutable { r.ty.unqual() } else { r.ty.unqual().with(QualConst) };
                    let ty = li.ty;
                    let lhs = texpr { kind: TExprLocal(l), ty, lvalue: true, span: lhs.span };
                    return texpr::rvalue(TExprAssign(bx(lhs), bx(r)), ty.unqual(), sp);
                }
            }
            if !self.is_let_init_target(&lhs) && !self.check_modifiable(&lhs, "assignment") {
                return texpr::error(sp);
            }
            let r = self.convert_assign(rhs, lhs.ty.unqual(), b.span, "assignment");
            let t = lhs.ty.unqual();
            return texpr::rvalue(TExprAssign(bx(lhs), bx(r)), t, sp);
        };
        if !self.check_modifiable(&lhs, "compound assignment") {
            return texpr::error(sp);
        }
        let r = self.to_rvalue(rhs);
        if r.is_error() {
            return texpr::error(sp);
        }
        let lt = lhs.ty.ty;
        if is_pointer(lt) && matches!(op, BinOpAdd | BinOpSub) && is_integer(r.ty.ty) {
            self.check_ptr_arith(&lhs, sp);
            let r = self.cast_to(r, q(ptrdiff_t()));
            let t = lhs.ty.unqual();
            return texpr::rvalue(TExprCompoundAssign(op, bx(lhs), bx(r), t), t, sp);
        }
        let ok = match op {
            BinOpRem | BinOpBitAnd | BinOpBitOr | BinOpBitXor | BinOpShl | BinOpShr => {
                is_integer(lt) && is_integer(r.ty.ty)
            }
            _ => is_arith(lt) && is_arith(r.ty.ty),
        };
        if !ok {
            diag::error(
                "CR1305",
                sp,
                format!(
                    "invalid operands to `{}=`: `{}` and `{}`",
                    op.spelling(),
                    display(lhs.ty),
                    display(r.ty)
                ),
            );
            return texpr::error(sp);
        }
        let op_ty =
            if matches!(op, BinOpShl | BinOpShr) { promote(lt) } else { usual_arith(lt, r.ty.ty) };
        let r = if matches!(op, BinOpShl | BinOpShr) {
            self.promote_expr(r)
        } else {
            self.cast_to(r, q(op_ty))
        };
        let t = lhs.ty.unqual();
        texpr::rvalue(TExprCompoundAssign(op, bx(lhs), bx(r), q(op_ty)), t, sp)
    }

    /// An immutable `let` (or a member of one) may be assigned once; the
    /// initialization analysis enforces "once" (CR0384).
    fn is_let_init_target(&self, e: &texpr) -> bool {
        match &e.kind {
            TExprLocal(l) => {
                let li = &self.fctx.as_ref().unwrap().locals[*l as usize];
                li.is_let && !li.mutable && !is_array(li.ty.ty)
            }
            TExprMember(b, r, i) => {
                !record(*r).is_union
                    && !record(*r).fields[*i as usize].ty.is_const()
                    && self.is_let_init_target(b)
            }
            _ => false,
        }
    }

    fn conditional(&mut self, c: &expr, a: &expr, b: &expr, sp: span) -> texpr {
        let cv = self.rvalue_expr(c);
        if !is_scalar(cv.ty.ty) && !cv.is_error() {
            diag::error(
                "CR1305",
                c.span,
                format!("condition has non-scalar type `{}`", display(cv.ty)),
            );
        }
        let x = self.rvalue_expr(a);
        let y = self.rvalue_expr(b);
        if x.is_error() || y.is_error() {
            return texpr::error(sp);
        }
        let (tx, ty_) = (x.ty.ty, y.ty.ty);
        let (x, y, t) = if is_arith(tx) && is_arith(ty_) {
            let (x, y, t) = self.arith_pair(x, y);
            (x, y, q(t))
        } else if is_void(tx) && is_void(ty_) {
            (x, y, q(void()))
        } else if compatible_unqual(tx, ty_) {
            let t = x.ty.unqual();
            (x, y, t)
        } else if is_pointer(tx) && self.is_null_constant(&y) {
            let t = x.ty;
            let y = self.cast_to(y, t);
            (x, y, t)
        } else if is_pointer(ty_) && self.is_null_constant(&x) {
            let t = y.ty;
            let x = self.cast_to(x, t);
            (x, y, t)
        } else if is_pointer(tx) && is_pointer(ty_) {
            let (pa, pb) = (pointee(tx).unwrap(), pointee(ty_).unwrap());
            let quals = pa.quals | pb.quals;
            let t = if is_void(pa.ty) || is_void(pb.ty) {
                q(pointer_to(q(void()).with(quals)))
            } else if compatible(pa.unqual(), pb.unqual()) {
                q(pointer_to(pa.with(quals)))
            } else {
                diag::error(
                    "CR1304",
                    sp,
                    format!(
                        "conditional operands have incompatible types `{}` and `{}`",
                        display(x.ty),
                        display(y.ty)
                    ),
                );
                return texpr::error(sp);
            };
            (self.cast_to(x, t), self.cast_to(y, t), t)
        } else {
            diag::error(
                "CR1304",
                sp,
                format!(
                    "conditional operands have incompatible types `{}` and `{}`",
                    display(x.ty),
                    display(y.ty)
                ),
            );
            return texpr::error(sp);
        };
        texpr::rvalue(TExprCond(bx(cv), bx(x), bx(y)), t, sp)
    }

    fn builtin_call(&mut self, name: sym, args: &[expr], sp: span) -> Option<texpr> {
        let n = name.as_str();
        let want = |k: usize| {
            if args.len() != k {
                diag::error("CR1307", sp, format!("`{n}` takes {k} argument(s)"));
            }
        };
        Some(match name {
            SymBuiltinVaStart => {
                want(2);
                let ap = self.expr(&args[0]);
                if !self.fctx.as_ref().is_some_and(|_| true) {
                    return Some(texpr::error(sp));
                }
                let ap = self.to_rvalue(ap);
                texpr::rvalue(TExprVaStart(bx(ap)), q(void()), sp)
            }
            SymBuiltinVaEnd => {
                want(1);
                let ap = self.rvalue_expr(&args[0]);
                texpr::rvalue(TExprVaEnd(bx(ap)), q(void()), sp)
            }
            SymBuiltinVaCopy => {
                want(2);
                let d = self.rvalue_expr(&args[0]);
                let s = self.rvalue_expr(&args[1]);
                texpr::rvalue(TExprVaCopy(bx(d), bx(s)), q(void()), sp)
            }
            SymBuiltinUnreachable => texpr::rvalue(TExprUnreachable, q(void()), sp),
            SymBuiltinTrap => texpr::rvalue(TExprTrap, q(void()), sp),
            SymBuiltinExpect => {
                want(2);
                let v = self.rvalue_expr(&args[0]);
                let _ = self.rvalue_expr(&args[1]);
                v
            }
            _ => return None,
        })
    }

    fn call(&mut self, f: &expr, args: &[expr], sp: span) -> texpr {
        if let ExprKindIdent(name) = &f.kind {
            if self.lookup(*name).is_none() {
                if let Some(r) = self.builtin_call(*name, args, sp) {
                    return r;
                }
            }
        }
        let callee = self.expr(f);
        let direct = match &callee.kind {
            TExprGlobal(g) if is_function(callee.ty.ty) => Some(*g),
            _ => None,
        };
        let callee = self.to_rvalue(callee);
        if callee.is_error() {
            for a in args {
                self.expr(a);
            }
            return texpr::error(sp);
        }
        let Some(ft) = func_of(callee.ty.ty).filter(|_| is_pointer(callee.ty.ty)) else {
            diag::error(
                "CR1316",
                f.span,
                format!("called object of type `{}` is not a function", display(callee.ty)),
            );
            return texpr::error(sp);
        };
        match direct {
            Some(g) => {
                let gd = &self.prog.globals[g as usize];
                let (name, unsafe_fn, c_fn) = (gd.name, gd.unsafe_fn, gd.c_origin && !gd.safe_attr);
                if unsafe_fn {
                    self.require_unsafe(sp, &format!("calling the unsafe function `{name}`"));
                } else if c_fn {
                    self.require_unsafe(sp, &format!("calling the C function `{name}`"));
                }
            }
            None => self.require_unsafe(sp, "calling through a function pointer"),
        }
        let np = ft.params.len();
        if args.len() < np || (args.len() > np && !ft.variadic) {
            diag::error("CR1307", sp, format!("expected {np} argument(s), found {}", args.len()));
        }
        let mut targs = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let v = self.expr(a);
            if i < np {
                let pt = ft.params[i];
                targs.push(self.convert_assign(v, pt.unqual(), a.span, "argument"));
            } else {
                let v = self.to_rvalue(v);
                let v = match kind(v.ty.ty) {
                    TypeKindFloat(FloatKindFloat) => self.cast_to(v, q(float_of(FloatKindDouble))),
                    TypeKindNullptr => self.cast_to(v, q(pointer_to(q(void())))),
                    _ if is_integer(v.ty.ty) => self.promote_expr(v),
                    _ => v,
                };
                targs.push(v);
            }
        }
        let rt = ft.ret.unqual();
        if !is_void(rt.ty) && !is_complete(rt.ty) {
            diag::error(
                "CR1310",
                sp,
                format!("calling a function that returns incomplete type `{}`", display(rt)),
            );
        }
        texpr::rvalue(TExprCall(bx(callee), targs, direct), rt, sp)
    }

    fn index(&mut self, a: &expr, i: &expr, sp: span) -> texpr {
        let base = self.expr(a);
        let idx = self.expr(i);
        if base.is_error() || idx.is_error() {
            return texpr::error(sp);
        }
        let (base, idx) =
            if is_integer(base.ty.ty) && (is_pointer(idx.ty.ty) || is_array(idx.ty.ty)) {
                (idx, base)
            } else {
                (base, idx)
            };
        let idx = self.to_rvalue(idx);
        if !is_integer(idx.ty.ty) {
            diag::error(
                "CR1305",
                i.span,
                format!("array subscript is not an integer: `{}`", display(idx.ty)),
            );
            return texpr::error(sp);
        }
        match base.ty.kind() {
            TypeKindArray(elem, n) if base.lvalue => {
                if let (Some(ConstValueInt(v)), Some(n)) = (consteval::eval(&idx), n) {
                    if v < 0 || v >= n as i128 {
                        diag::error(
                            "CR0701",
                            i.span,
                            format!("index {v} is out of bounds for an array of length {n}"),
                        )
                        .label(format!("the array has {n} element(s)"));
                    }
                }
                let ty = elem.with(base.ty.quals);
                texpr { kind: TExprIndex(bx(base), bx(idx)), ty, lvalue: true, span: sp }
            }
            TypeKindSlice { mutable, elem, .. } => {
                let b = self.to_rvalue(base);
                let ty = if mutable { elem } else { elem.with(QualConst) };
                texpr { kind: TExprSliceIndex(bx(b), bx(idx)), ty, lvalue: true, span: sp }
            }
            TypeKindRef { mutable, pointee, .. } if is_array(pointee.ty) => {
                let b = self.to_rvalue(base);
                let d = self.deref(b, sp);
                let elem = match pointee.kind() {
                    TypeKindArray(e, _) => e,
                    _ => unreachable!(),
                };
                let ty = if mutable { elem } else { elem.with(QualConst) };
                texpr { kind: TExprIndex(bx(d), bx(idx)), ty, lvalue: true, span: sp }
            }
            _ => {
                let p = self.to_rvalue(base);
                if !is_pointer(p.ty.ty) {
                    diag::error(
                        "CR1305",
                        sp,
                        format!(
                            "subscripted value of type `{}` is not an array, slice, or pointer",
                            display(p.ty)
                        ),
                    );
                    return texpr::error(sp);
                }
                self.check_ptr_arith(&p, sp);
                let t = p.ty;
                let sum = texpr::rvalue(TExprPtrAdd(bx(p), bx(idx)), t, sp);
                self.deref(sum, sp)
            }
        }
    }

    /// Finds a member, searching anonymous members. Returns the path of
    /// (record, field index) steps.
    fn find_member(&self, rec: u32, name: sym) -> Option<Vec<(u32, u32)>> {
        let r = record(rec);
        for (i, f) in r.fields.iter().enumerate() {
            if f.name == Some(name) {
                return Some(vec![(rec, i as u32)]);
            }
            if f.name.is_none() {
                if let TypeKindRecord(inner) = f.ty.kind() {
                    if let Some(mut p) = self.find_member(inner, name) {
                        p.insert(0, (rec, i as u32));
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    fn member(&mut self, base: &expr, name: sym, nsp: span, arrow: bool, sp: span) -> texpr {
        let mut b = self.expr(base);
        if b.is_error() {
            return b;
        }
        if arrow {
            let p = self.to_rvalue(b);
            if !matches!(p.ty.kind(), TypeKindPointer(_) | TypeKindRef { .. }) {
                diag::error(
                    "CR1305",
                    sp,
                    format!("`->` applied to non-pointer type `{}`", display(p.ty)),
                );
                return texpr::error(sp);
            }
            b = self.deref(p, sp);
            if b.is_error() {
                return b;
            }
        }
        if let TypeKindSlice { .. } = b.ty.kind() {
            if name == SymLen {
                let v = self.to_rvalue(b);
                return texpr::rvalue(TExprSliceLen(bx(v)), q(size_t()), sp);
            }
            diag::error("CR1309", nsp, format!("a slice has no member `{name}`; it has `len`"));
            return texpr::error(sp);
        }
        let TypeKindRecord(rec) = b.ty.kind() else {
            let hint =
                if is_pointer(b.ty.ty) || is_ref(b.ty.ty) { "; did you mean `->`?" } else { "" };
            diag::error(
                "CR1305",
                sp,
                format!("member access on non-struct type `{}`{hint}", display(b.ty)),
            );
            return texpr::error(sp);
        };
        if !record(rec).complete {
            diag::error(
                "CR1310",
                sp,
                format!("member access into incomplete type `{}`", display(b.ty)),
            );
            return texpr::error(sp);
        }
        let Some(path) = self.find_member(rec, name) else {
            diag::error(
                "CR1309",
                nsp,
                format!("`{}` has no member named `{name}`", display(b.ty.unqual())),
            );
            return texpr::error(sp);
        };
        let mut cur = b;
        for (r, i) in path {
            let f = record(r).fields[i as usize].clone();
            let ty = f.ty.with(cur.ty.quals);
            let lvalue = cur.lvalue;
            cur = texpr { kind: TExprMember(bx(cur), r, i), ty, lvalue, span: sp };
        }
        cur
    }

    fn compound_literal(&mut self, tn: &type_name, init: &Initializer, sp: span) -> texpr {
        let t = self.type_name(tn);
        if self.fctx.is_none() {
            let mut gd = self.new_global(SymEmpty, t, false, sp);
            gd.linkage = LinkageInternal;
            gd.symbol = format!(".compound.{}", self.prog.globals.len());
            gd.defined = true;
            self.prog.globals.push(gd);
            let g = self.prog.globals.len() as global_id - 1;
            self.global_init(g, init, false);
            let ty = self.prog.globals[g as usize].ty;
            return texpr { kind: TExprGlobal(g), ty, lvalue: true, span: sp };
        }
        let (ti, ty) = self.local_init(t, init);
        let lid = self.new_local(SymEmpty, ty, sp, true, false);
        self.fctx.as_mut().unwrap().locals[lid as usize].is_temp = true;
        texpr { kind: TExprCompound(lid, Box::new(ti)), ty, lvalue: true, span: sp }
    }

    fn generic(&mut self, ctrl: &expr, assocs: &[(Option<type_name>, expr)], sp: span) -> texpr {
        self.unevaluated += 1;
        let c = self.rvalue_expr(ctrl);
        self.unevaluated -= 1;
        let ct = c.ty.unqual();
        let mut chosen = None;
        let mut default = None;
        for (tn, e) in assocs {
            match tn {
                None => default = Some(e),
                Some(tn) => {
                    let t = self.type_name(tn);
                    if chosen.is_none() && compatible(t.unqual(), ct) {
                        chosen = Some(e);
                    }
                }
            }
        }
        match chosen.or(default) {
            Some(e) => self.expr(e),
            None => {
                diag::error(
                    "CR1304",
                    sp,
                    format!("no `_Generic` association matches `{}`", display(ct)),
                );
                texpr::error(sp)
            }
        }
    }

    fn borrow(&mut self, mutable: bool, x: &expr, sp: span) -> texpr {
        let v = self.expr(x);
        if v.is_error() {
            return v;
        }
        if !v.lvalue {
            diag::error(
                "CR1306",
                sp,
                "can only borrow a place (a variable, a member, an element, or a dereference)",
            )
            .help("store the value in a variable first, then borrow the variable");
            return texpr::error(sp);
        }
        if is_function(v.ty.ty) {
            diag::error("CR1321", sp, "cannot borrow a function");
            return texpr::error(sp);
        }
        if mutable && (v.ty.is_const() || self.has_const_member(v.ty.ty)) {
            let mut cur = &v;
            loop {
                match &cur.kind {
                    TExprLocal(l) => {
                        let li = &self.fctx.as_ref().unwrap().locals[*l as usize];
                        diag::error(
                            "CR0596",
                            sp,
                            format!("cannot borrow `{}` as mutable", li.name),
                        )
                        .secondary(li.span, "declared here without `mut`")
                        .help(format!("make the binding mutable: `let mut {}`", li.name));
                        return texpr::error(sp);
                    }
                    TExprMember(b, _, _) | TExprIndex(b, _) => cur = b,
                    TExprDeref(_) | TExprSliceIndex(..) => {
                        diag::error(
                            "CR0596",
                            sp,
                            "cannot borrow data behind a shared borrow or `const` pointer as mutable",
                        );
                        return texpr::error(sp);
                    }
                    _ => break,
                }
            }
            diag::error("CR0596", sp, "cannot borrow a read-only value as mutable");
            return texpr::error(sp);
        }
        if let TExprMember(_, r, i) = &v.kind {
            if record(*r).fields[*i as usize].bits.is_some() {
                diag::error("CR1306", sp, "cannot borrow a bit-field");
                return texpr::error(sp);
            }
        }
        let pointee = if mutable { v.ty } else { v.ty.unqual().with(v.ty.quals & !QualConst) };
        let t = q(intern(TypeKindRef { mutable, lifetime: None, pointee }));
        texpr::rvalue(TExprRef(mutable, bx(v)), t, sp)
    }

    fn offsetof(&mut self, tn: &type_name, path: &[Designator], sp: span) -> texpr {
        let mut t = self.type_name(tn);
        let mut off: u64 = 0;
        for d in path {
            match d {
                DesignatorField(name, nsp) => {
                    let TypeKindRecord(r) = t.kind() else {
                        diag::error(
                            "CR1309",
                            *nsp,
                            "`offsetof` member path goes through a non-struct type",
                        );
                        return texpr::error(sp);
                    };
                    let Some(p) = self.find_member(r, *name) else {
                        diag::error("CR1309", *nsp, format!("no member named `{name}`"));
                        return texpr::error(sp);
                    };
                    for (rr, i) in p {
                        let f = record(rr).fields[i as usize].clone();
                        off += f.offset;
                        t = f.ty;
                    }
                }
                DesignatorIndex(e, _) => {
                    let v = self.const_int_expr(e).unwrap_or(0);
                    let TypeKindArray(elem, _) = t.kind() else {
                        diag::error("CR1309", sp, "`offsetof` index applied to a non-array type");
                        return texpr::error(sp);
                    };
                    off = off.wrapping_add((v as u64).wrapping_mul(size_of(elem.ty)));
                    t = elem;
                }
            }
        }
        texpr::int_const(off as i128, size_t(), sp)
    }
}

/// Where the commonest standard functions are declared, for a fix hint.
fn std_header_hint(name: sym) -> Option<&'static str> {
    Some(match name.as_str() {
        "printf" | "puts" | "putchar" | "fprintf" | "sprintf" | "snprintf" | "fopen" | "fclose"
        | "fgets" | "fputs" | "scanf" | "getchar" | "stdout" | "stderr" | "stdin" | "perror" => {
            "stdio.h"
        }
        "malloc" | "free" | "calloc" | "realloc" | "exit" | "abort" | "atoi" | "qsort"
        | "getenv" => "stdlib.h",
        "strlen" | "strcmp" | "strcpy" | "strncpy" | "memcpy" | "memset" | "memmove" | "strcat"
        | "strchr" => "string.h",
        "size_t" | "NULL" | "offsetof" => "stddef.h",
        "sqrt" | "pow" | "sin" | "cos" | "fabs" | "floor" | "ceil" => "math.h",
        "assert" => "assert.h",
        _ => return None,
    })
}

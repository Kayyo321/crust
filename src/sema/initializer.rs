/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::consteval;
use super::tir::*;
use super::types::*;
use super::*;
use crate::diag;

type init_items = Vec<(Vec<PathElem>, texpr)>;

struct cursor<'b> {
    items: &'b [(Vec<Designator>, Initializer)],
    pos: usize,
}

fn elem_path(path: &[PathElem], e: PathElem) -> Vec<PathElem> {
    let mut p = path.to_vec();
    p.push(e);
    p
}

impl sema<'_> {
    /// Resolves an initializer for a local object of type `ty`. Returns the
    /// initializer and the type, completed if it was an array of unknown size.
    pub fn local_init(&mut self, ty: qual_type, init: &Initializer) -> (tinit, qual_type) {
        match init {
            InitializerExpr(e) => {
                if let Some(r) = self.string_array_init(ty, e) {
                    return r;
                }
                if is_array(ty.ty) {
                    diag::error(
                        "CR1311",
                        e.span,
                        "an array must be initialized with a braced list or a string literal",
                    );
                    return (TInitList { zero: true, items: vec![] }, ty);
                }
                let v = self.expr(e);
                let v = self.convert_assign(v, ty.unqual(), e.span, "initializer");
                (TInitExpr(v), ty)
            }
            InitializerList(items, sp) => {
                let mut out = Vec::new();
                let n = self.init_braced(ty, &[], items, *sp, &mut out);
                let ty = match ty.kind() {
                    TypeKindArray(e, None) => {
                        qual_type { ty: array_of(e, Some(n)), quals: ty.quals }
                    }
                    _ => ty,
                };
                (TInitList { zero: true, items: out }, ty)
            }
        }
    }

    /// `char s[] = "abc";` and friends.
    fn string_array_init(&mut self, ty: qual_type, e: &expr) -> Option<(tinit, qual_type)> {
        let TypeKindArray(elem, n) = ty.kind() else { return None };
        let inner = match &e.kind {
            ExprKindString(_) => e,
            _ => return None,
        };
        let s = self.expr(inner);
        let TExprString(id) = s.kind else { return None };
        let lit = self.prog.strings[id as usize].clone();
        let unit = lit.encoding.unit_size();
        if size_of(elem.ty) != unit || !is_integer(elem.ty) {
            return None;
        }
        let len = lit.units.len() as u64;
        let total = n.unwrap_or(len + 1);
        if let Some(n) = n {
            if len > n {
                diag::error(
                    "CR1311",
                    e.span,
                    format!("string of length {len} does not fit an array of {n} elements"),
                );
            }
        }
        let ty = qual_type { ty: array_of(elem, Some(total)), quals: ty.quals };
        let mut items = Vec::new();
        for (i, u) in lit.units.iter().enumerate().take(total as usize) {
            items.push((
                vec![PathElemIndex(i as u64)],
                texpr::int_const(*u as i128, elem.ty, e.span),
            ));
        }
        Some((TInitList { zero: true, items }, ty))
    }

    /// Initializes `ty` from a braced list. Returns the number of elements
    /// (for completing an array of unknown size).
    fn init_braced(
        &mut self,
        ty: qual_type,
        path: &[PathElem],
        items: &[(Vec<Designator>, Initializer)],
        sp: span,
        out: &mut init_items,
    ) -> u64 {
        if is_scalar(ty.ty) || is_ref(ty.ty) || is_slice(ty.ty) {
            if items.is_empty() {
                return 0;
            }
            if items.len() > 1 {
                diag::warning("CR1311", sp, "excess elements in scalar initializer");
            }
            let (d, i) = &items[0];
            if !d.is_empty() {
                diag::error("CR1311", sp, "a designator cannot be used with a scalar");
            }
            self.init_object(ty, path, i, out);
            return 1;
        }
        let mut cur = cursor { items, pos: 0 };
        let n = self.init_aggregate(ty, path, &mut cur, out, true);
        if cur.pos < items.len() {
            diag::error("CR1311", items[cur.pos].1.span(), "excess elements in initializer");
        }
        n
    }

    fn init_object(
        &mut self,
        ty: qual_type,
        path: &[PathElem],
        init: &Initializer,
        out: &mut init_items,
    ) {
        match init {
            InitializerList(items, sp) => {
                self.init_braced(ty, path, items, *sp, out);
            }
            InitializerExpr(e) => {
                if let Some((TInitList { items, .. }, _)) = self.string_array_init(ty, e) {
                    for (p, v) in items {
                        let mut full = path.to_vec();
                        full.extend(p);
                        out.push((full, v));
                    }
                    return;
                }
                if is_array(ty.ty) {
                    diag::error(
                        "CR1311",
                        e.span,
                        "an array must be initialized with a braced list or a string literal",
                    );
                    return;
                }
                let v = self.expr(e);
                let v = self.convert_assign(v, ty.unqual(), e.span, "initializer");
                out.push((path.to_vec(), v));
            }
        }
    }

    /// The subobjects of an aggregate: (path element, type).
    fn subobjects(&self, ty: qual_type) -> (Vec<(PathElem, qual_type)>, bool) {
        match ty.kind() {
            TypeKindArray(e, n) => {
                let e = e.with(ty.quals);
                match n {
                    Some(n) => ((0..n).map(|i| (PathElemIndex(i), e)).collect(), false),
                    None => (Vec::new(), true),
                }
            }
            TypeKindRecord(r) => {
                let rd = record(r);
                let mut v = Vec::new();
                for (i, f) in rd.fields.iter().enumerate() {
                    if f.name.is_none() && f.bits.is_some() {
                        continue;
                    }
                    if let TypeKindArray(_, None) = f.ty.kind() {
                        continue;
                    }
                    v.push((PathElemField(r, i as u32), f.ty.with(ty.quals)));
                    if rd.is_union {
                        break;
                    }
                }
                (v, false)
            }
            _ => (Vec::new(), false),
        }
    }

    fn init_aggregate(
        &mut self,
        ty: qual_type,
        path: &[PathElem],
        cur: &mut cursor,
        out: &mut init_items,
        top: bool,
    ) -> u64 {
        let (subs, unbounded) = self.subobjects(ty);
        let elem_ty = match ty.kind() {
            TypeKindArray(e, _) => Some(e.with(ty.quals)),
            _ => None,
        };
        let mut i: u64 = 0;
        let mut max: u64 = 0;
        while cur.pos < cur.items.len() {
            let (desig, init) = &cur.items[cur.pos];
            if !desig.is_empty() {
                if !top {
                    break;
                }
                cur.pos += 1;
                let Some((pes, sub_ty, idx)) = self.designate(ty, &desig[0]) else { continue };
                let mut p = path.to_vec();
                p.extend(pes);
                let mut tmp = Vec::new();
                self.init_designated_into(sub_ty, &p, &desig[1..], init, &mut tmp);
                out.extend(tmp);
                i = idx + 1;
                max = max.max(i);
                continue;
            }
            let (pe, sub_ty) = if (i as usize) < subs.len() {
                subs[i as usize].clone()
            } else if unbounded {
                (PathElemIndex(i), elem_ty.unwrap())
            } else {
                break;
            };
            let p = elem_path(path, pe);
            match init {
                InitializerList(..) => {
                    cur.pos += 1;
                    self.init_object(sub_ty, &p, init, out);
                }
                InitializerExpr(e) => {
                    let aggregate = is_array(sub_ty.ty) || is_record(sub_ty.ty);
                    if aggregate {
                        let is_string = matches!(e.kind, ExprKindString(_)) && is_array(sub_ty.ty);
                        let whole = !is_string && {
                            self.unevaluated += 1;
                            let v = self.expr(e);
                            self.unevaluated -= 1;
                            compatible_unqual(v.ty.ty, sub_ty.ty)
                        };
                        if is_string || whole {
                            cur.pos += 1;
                            self.init_object(sub_ty, &p, init, out);
                        } else {
                            self.init_aggregate(sub_ty, &p, cur, out, false);
                        }
                    } else {
                        cur.pos += 1;
                        self.init_object(sub_ty, &p, init, out);
                    }
                }
            }
            i += 1;
            max = max.max(i);
            if matches!(ty.kind(), TypeKindRecord(r) if record(r).is_union) {
                break;
            }
        }
        max
    }

    fn init_designated_into(
        &mut self,
        ty: qual_type,
        path: &[PathElem],
        rest: &[Designator],
        init: &Initializer,
        out: &mut init_items,
    ) {
        if rest.is_empty() {
            self.init_object(ty, path, init, out);
            return;
        }
        let Some((pes, sub, _)) = self.designate(ty, &rest[0]) else { return };
        let mut p = path.to_vec();
        p.extend(pes);
        self.init_designated_into(sub, &p, &rest[1..], init, out);
    }

    /// Resolves one designator against `ty`: the path step, the subobject
    /// type, and the subobject's position.
    fn designate(
        &mut self,
        ty: qual_type,
        d: &Designator,
    ) -> Option<(Vec<PathElem>, qual_type, u64)> {
        match (d, ty.kind()) {
            (DesignatorField(name, sp), TypeKindRecord(r)) => {
                // Members of anonymous structs and unions are found through
                // them, so the path can have several steps.
                fn find(r: u32, name: sym) -> Option<Vec<(u32, u32)>> {
                    let rd = record(r);
                    for (i, f) in rd.fields.iter().enumerate() {
                        if f.name == Some(name) {
                            return Some(vec![(r, i as u32)]);
                        }
                        if f.name.is_none() {
                            if let TypeKindRecord(inner) = f.ty.kind() {
                                if let Some(mut p) = find(inner, name) {
                                    p.insert(0, (r, i as u32));
                                    return Some(p);
                                }
                            }
                        }
                    }
                    None
                }
                if let Some(steps) = find(r, *name) {
                    let top = steps[0].1 as u64;
                    let (lr, li) = *steps.last().unwrap();
                    let fty = record(lr).fields[li as usize].ty.with(ty.quals);
                    let pes = steps.iter().map(|(rr, ii)| PathElemField(*rr, *ii)).collect();
                    return Some((pes, fty, top));
                }
                diag::error(
                    "CR1311",
                    *sp,
                    format!("`{}` has no member named `{name}`", display(ty.unqual())),
                );
                None
            }
            (DesignatorIndex(e, sp), TypeKindArray(elem, n)) => {
                let v = self.const_int_expr(e)?;
                if v < 0 || n.is_some_and(|n| v as u64 >= n) {
                    diag::error(
                        "CR1311",
                        *sp,
                        format!("array designator index {v} is out of bounds"),
                    );
                    return None;
                }
                Some((vec![PathElemIndex(v as u64)], elem.with(ty.quals), v as u64))
            }
            (DesignatorField(_, sp), _) => {
                diag::error("CR1311", *sp, "field designator used with a non-struct type");
                None
            }
            (DesignatorIndex(_, sp), _) => {
                diag::error("CR1311", *sp, "array designator used with a non-array type");
                None
            }
        }
    }

    // ---- static initializers ----

    /// Resolves and constant-evaluates the initializer of a static object.
    pub fn global_init(&mut self, g: global_id, init: &Initializer, is_const: bool) {
        let ty = self.prog.globals[g as usize].ty;
        let saved = self.fctx.take();
        // Initializers of statics are evaluated with no function context, so
        // compound literals inside them become statics too.
        let (ti, ty) = self.local_init(ty, init);
        self.fctx = saved;
        self.prog.globals[g as usize].ty = ty;
        if !is_complete(ty.ty) {
            diag::error(
                "CR1310",
                init.span(),
                format!("variable has incomplete type `{}`", display(ty)),
            );
            return;
        }
        let mut data = const_data { bytes: vec![0; size_of(ty.ty) as usize], relocs: Vec::new() };
        let code = if self.module.is_some() { "CR1413" } else { "CR1308" };
        match &ti {
            TInitExpr(e) => {
                if is_const && is_scalar(ty.ty) {
                    if let Some(v) = consteval::eval(e) {
                        self.prog.globals[g as usize].const_value = Some(v);
                    }
                }
                if !self.write_const(&mut data, 0, ty, e) {
                    diag::error(code, e.span, "initializer element is not a compile-time constant");
                }
            }
            TInitList { items, .. } => {
                for (path, e) in items {
                    let (off, t, bits) = path_offset(ty, path);
                    let ok = match bits {
                        Some(bf) => match consteval::eval(e) {
                            Some(ConstValueInt(v)) => {
                                write_bits(&mut data, off, size_of(t.ty), bf.shift, bf.width, v);
                                true
                            }
                            _ => false,
                        },
                        None => self.write_const(&mut data, off, t, e),
                    };
                    if !ok {
                        diag::error(
                            code,
                            e.span,
                            "initializer element is not a compile-time constant",
                        );
                    }
                }
            }
        }
        self.prog.globals[g as usize].init = Some(data);
    }

    /// Writes a constant into the image. Returns false if it is not constant.
    fn write_const(&mut self, data: &mut const_data, off: u64, t: qual_type, e: &texpr) -> bool {
        if is_record(t.ty) || is_array(t.ty) {
            // Copy from another constant object, e.g. a compound literal.
            if let TExprLoad(inner) = &e.kind {
                if let TExprGlobal(src) = inner.kind {
                    if let Some(init) = self.prog.globals[src as usize].init.clone() {
                        let n = init.bytes.len().min(data.bytes.len() - off as usize);
                        data.bytes[off as usize..off as usize + n]
                            .copy_from_slice(&init.bytes[..n]);
                        for r in init.relocs {
                            data.relocs.push(reloc {
                                offset: r.offset + off,
                                target: r.target,
                                addend: r.addend,
                            });
                        }
                        return true;
                    }
                }
            }
            return false;
        }
        if let Some(v) = consteval::eval(e) {
            let size = size_of(t.ty) as usize;
            let bytes: Vec<u8> = match (v, kind(t.ty)) {
                (ConstValueFloat(f), TypeKindFloat(FloatKindFloat)) => {
                    (f as f32).to_le_bytes().to_vec()
                }
                (ConstValueFloat(f), TypeKindFloat(FloatKindDouble)) => f.to_le_bytes().to_vec(),
                (ConstValueFloat(f), TypeKindFloat(FloatKindLongDouble)) => f64_to_x87(f).to_vec(),
                (ConstValueInt(i), TypeKindFloat(_)) => {
                    let f = i as f64;
                    return self.write_const(
                        data,
                        off,
                        t,
                        &texpr::rvalue(TExprFloat(f), t, e.span),
                    );
                }
                (ConstValueInt(i), _) => i.to_le_bytes()[..size.min(16)].to_vec(),
                (ConstValueFloat(f), _) => (f as i128).to_le_bytes()[..size.min(16)].to_vec(),
            };
            let o = off as usize;
            data.bytes[o..o + bytes.len().min(size)]
                .copy_from_slice(&bytes[..bytes.len().min(size)]);
            return true;
        }
        if let Some((target, addend)) = self.address_constant(e) {
            data.relocs.push(reloc { offset: off, target, addend });
            return true;
        }
        false
    }

    /// An address constant: `&global`, a string literal, a function, plus an
    /// optional constant byte offset.
    fn address_constant(&self, e: &texpr) -> Option<(RelocTarget, i64)> {
        match &e.kind {
            TExprCast(
                CastKindArrayDecay | CastKindFuncDecay | CastKindPtrToPtr | CastKindNoop
                | CastKindIntToInt | CastKindPtrToInt,
                inner,
            ) => self.address_constant(inner).or_else(|| self.lvalue_address(inner)),
            TExprAddrOf(inner) => self.lvalue_address(inner),
            TExprPtrAdd(p, i) => {
                let (t, a) = self.address_constant(p)?;
                let ConstValueInt(n) = consteval::eval(i)? else { return None };
                let sz = pointee(p.ty.ty).map(|t| size_of(t.ty)).unwrap_or(1) as i64;
                Some((t, a + n as i64 * sz))
            }
            _ => None,
        }
    }

    fn lvalue_address(&self, e: &texpr) -> Option<(RelocTarget, i64)> {
        match &e.kind {
            TExprGlobal(g) => Some((RelocTargetGlobal(*g), 0)),
            TExprString(s) => Some((RelocTargetString(*s), 0)),
            TExprMember(b, r, i) => {
                let (t, a) = self.lvalue_address(b)?;
                Some((t, a + record(*r).fields[*i as usize].offset as i64))
            }
            TExprIndex(b, i) => {
                let (t, a) = self.lvalue_address(b)?;
                let ConstValueInt(n) = consteval::eval(i)? else { return None };
                Some((t, a + n as i64 * size_of(e.ty.ty) as i64))
            }
            TExprDeref(p) => self.address_constant(p),
            _ => None,
        }
    }
}

/// Byte offset, type, and bit-field info of the subobject at `path`.
pub fn path_offset(ty: qual_type, path: &[PathElem]) -> (u64, qual_type, Option<bit_field>) {
    let mut off = 0;
    let mut t = ty;
    let mut bits = None;
    for p in path {
        match p {
            PathElemField(r, i) => {
                let f = record(*r).fields[*i as usize].clone();
                off += f.offset;
                t = f.ty;
                bits = f.bits;
            }
            PathElemIndex(i) => {
                let TypeKindArray(e, _) = t.kind() else { break };
                off += i * size_of(e.ty);
                t = e;
                bits = None;
            }
        }
    }
    (off, t, bits)
}

fn write_bits(data: &mut const_data, off: u64, unit: u64, shift: u32, width: u32, v: i128) {
    let o = off as usize;
    let n = unit as usize;
    let mut cur: u128 = 0;
    for k in 0..n {
        cur |= (data.bytes[o + k] as u128) << (8 * k);
    }
    let mask: u128 = if width >= 128 { u128::MAX } else { ((1u128 << width) - 1) << shift };
    cur = (cur & !mask) | (((v as u128) << shift) & mask);
    for k in 0..n {
        data.bytes[o + k] = (cur >> (8 * k)) as u8;
    }
}

/// Converts an f64 to the x87 80-bit extended format, padded to 16 bytes.
pub fn f64_to_x87(f: f64) -> [u8; 16] {
    let mut out = [0u8; 16];
    let bits = f.to_bits();
    let sign = (bits >> 63) as u16;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (e80, m80): (u16, u64) = if exp == 0 && frac == 0 {
        (0, 0)
    } else if exp == 0x7ff {
        (0x7fff, (1u64 << 63) | (frac << 11))
    } else if exp == 0 {
        // Subnormal double: normalize.
        let lz = frac.leading_zeros() as i32 - 11;
        let m = frac << (lz + 11 + 1);
        ((1 - 1023 - lz - 1 + 16383) as u16, m | (1u64 << 63))
    } else {
        ((exp - 1023 + 16383) as u16, (1u64 << 63) | (frac << 11))
    };
    out[..8].copy_from_slice(&m80.to_le_bytes());
    let se = (sign << 15) | e80;
    out[8..10].copy_from_slice(&se.to_le_bytes());
    out
}

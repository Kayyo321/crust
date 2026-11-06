/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use crate::diag::span;
use crate::lex::sym;
use crate::parse::ast::{QualAtomic, QualConst, QualRestrict, QualVolatile};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct type_id(pub u32);

/// A type with its qualifiers (`const`, `volatile`, `restrict`, `_Atomic`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct qual_type {
    pub ty: type_id,
    pub quals: u8,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum IntKind {
    IntKindChar,
    IntKindSChar,
    IntKindUChar,
    IntKindShort,
    IntKindUShort,
    IntKindInt,
    IntKindUInt,
    IntKindLong,
    IntKindULong,
    IntKindLongLong,
    IntKindULongLong,
    IntKindInt128,
    IntKindUInt128,
}
pub use IntKind::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum FloatKind {
    FloatKindFloat,
    FloatKindDouble,
    FloatKindLongDouble,
}
pub use FloatKind::*;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct func_type {
    pub ret: qual_type,
    pub params: Vec<qual_type>,
    pub variadic: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TypeKind {
    TypeKindVoid,
    TypeKindBool,
    TypeKindInt(IntKind),
    TypeKindFloat(FloatKind),
    TypeKindPointer(qual_type),
    TypeKindArray(qual_type, Option<u64>),
    TypeKindFunction(func_type),
    TypeKindRecord(u32),
    TypeKindEnum(u32),
    /// A borrow: `ref 'a T` or `ref 'a mut T`.
    TypeKindRef {
        mutable: bool,
        lifetime: Option<sym>,
        pointee: qual_type,
    },
    /// A slice: `ref 'a [T]` or `ref 'a mut [T]`. Lowered as `{ T *ptr; size_t len; }`.
    TypeKindSlice {
        mutable: bool,
        lifetime: Option<sym>,
        elem: qual_type,
    },
    TypeKindNullptr,
    /// Poison type after an error, so one mistake gives one diagnostic.
    TypeKindError,
}
pub use TypeKind::*;

#[derive(Clone, Debug)]
pub struct bit_field {
    /// Bit position of the field inside its storage unit.
    pub shift: u32,
    pub width: u32,
}

#[derive(Clone, Debug)]
pub struct field {
    pub name: Option<sym>,
    pub ty: qual_type,
    /// Byte offset. For a bit-field, the offset of its storage unit.
    pub offset: u64,
    pub bits: Option<bit_field>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct drop_info {
    /// Symbol of the drop function, and the global it names in the defining unit.
    pub symbol: String,
    pub name: sym,
}

#[derive(Clone, Debug)]
pub struct record_def {
    pub is_union: bool,
    pub tag: Option<sym>,
    /// Module that defined the record, for module-qualified identity.
    pub module: Option<sym>,
    pub complete: bool,
    pub fields: Vec<field>,
    pub size: u64,
    pub align: u64,
    pub drop: Option<drop_info>,
    /// Has a drop function, or contains an owned member.
    pub owned: bool,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct enum_def {
    pub tag: Option<sym>,
    pub module: Option<sym>,
    pub complete: bool,
    pub underlying: IntKind,
    pub fixed: bool,
    pub enumerators: Vec<(sym, i128)>,
    pub span: span,
}

#[derive(Default)]
struct type_table {
    kinds: Vec<TypeKind>,
    map: HashMap<TypeKind, type_id>,
    records: Vec<record_def>,
    enums: Vec<enum_def>,
}

thread_local! {
    static Table: RefCell<type_table> = RefCell::new(type_table::default());
}

pub fn intern(k: TypeKind) -> type_id {
    Table.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(&id) = t.map.get(&k) {
            return id;
        }
        let id = type_id(t.kinds.len() as u32);
        t.kinds.push(k.clone());
        t.map.insert(k, id);
        id
    })
}

pub fn kind(t: type_id) -> TypeKind {
    Table.with(|tb| tb.borrow().kinds[t.0 as usize].clone())
}

pub fn new_record(r: record_def) -> u32 {
    Table.with(|t| {
        let mut t = t.borrow_mut();
        t.records.push(r);
        t.records.len() as u32 - 1
    })
}

pub fn record(id: u32) -> record_def {
    Table.with(|t| t.borrow().records[id as usize].clone())
}

pub fn with_record<R>(id: u32, f: impl FnOnce(&record_def) -> R) -> R {
    Table.with(|t| f(&t.borrow().records[id as usize]))
}

pub fn update_record(id: u32, f: impl FnOnce(&mut record_def)) {
    Table.with(|t| f(&mut t.borrow_mut().records[id as usize]))
}

pub fn new_enum(e: enum_def) -> u32 {
    Table.with(|t| {
        let mut t = t.borrow_mut();
        t.enums.push(e);
        t.enums.len() as u32 - 1
    })
}

pub fn enum_info(id: u32) -> enum_def {
    Table.with(|t| t.borrow().enums[id as usize].clone())
}

pub fn update_enum(id: u32, f: impl FnOnce(&mut enum_def)) {
    Table.with(|t| f(&mut t.borrow_mut().enums[id as usize]))
}

// ---- constructors ----

pub fn q(ty: type_id) -> qual_type {
    qual_type { ty, quals: 0 }
}

pub fn void() -> type_id {
    intern(TypeKindVoid)
}
pub fn bool_() -> type_id {
    intern(TypeKindBool)
}
pub fn int_of(k: IntKind) -> type_id {
    intern(TypeKindInt(k))
}
pub fn int() -> type_id {
    int_of(IntKindInt)
}
pub fn char_() -> type_id {
    int_of(IntKindChar)
}
pub fn size_t() -> type_id {
    int_of(IntKindULong)
}
pub fn ptrdiff_t() -> type_id {
    int_of(IntKindLong)
}
pub fn float_of(k: FloatKind) -> type_id {
    intern(TypeKindFloat(k))
}
pub fn error() -> type_id {
    intern(TypeKindError)
}
pub fn pointer_to(t: qual_type) -> type_id {
    intern(TypeKindPointer(t))
}
pub fn array_of(t: qual_type, n: Option<u64>) -> type_id {
    intern(TypeKindArray(t, n))
}

impl qual_type {
    pub fn kind(self) -> TypeKind {
        kind(self.ty)
    }
    pub fn is_const(self) -> bool {
        self.quals & QualConst != 0
    }
    pub fn with(self, quals: u8) -> qual_type {
        qual_type { ty: self.ty, quals: self.quals | quals }
    }
    pub fn unqual(self) -> qual_type {
        qual_type { ty: self.ty, quals: 0 }
    }
}

// ---- predicates ----

pub fn is_void(t: type_id) -> bool {
    matches!(kind(t), TypeKindVoid)
}
pub fn is_error(t: type_id) -> bool {
    matches!(kind(t), TypeKindError)
}
pub fn is_integer(t: type_id) -> bool {
    matches!(kind(t), TypeKindInt(_) | TypeKindBool | TypeKindEnum(_))
}
pub fn is_float(t: type_id) -> bool {
    matches!(kind(t), TypeKindFloat(_))
}
pub fn is_arith(t: type_id) -> bool {
    is_integer(t) || is_float(t)
}
pub fn is_pointer(t: type_id) -> bool {
    matches!(kind(t), TypeKindPointer(_))
}
pub fn is_scalar(t: type_id) -> bool {
    is_arith(t) || matches!(kind(t), TypeKindPointer(_) | TypeKindNullptr)
}
pub fn is_record(t: type_id) -> bool {
    matches!(kind(t), TypeKindRecord(_))
}
pub fn is_array(t: type_id) -> bool {
    matches!(kind(t), TypeKindArray(..))
}
pub fn is_function(t: type_id) -> bool {
    matches!(kind(t), TypeKindFunction(_))
}
pub fn is_ref(t: type_id) -> bool {
    matches!(kind(t), TypeKindRef { .. })
}
pub fn is_slice(t: type_id) -> bool {
    matches!(kind(t), TypeKindSlice { .. })
}
pub fn pointee(t: type_id) -> Option<qual_type> {
    match kind(t) {
        TypeKindPointer(p) => Some(p),
        TypeKindRef { pointee, .. } => Some(pointee),
        _ => None,
    }
}
pub fn func_of(t: type_id) -> Option<func_type> {
    match kind(t) {
        TypeKindFunction(f) => Some(f),
        TypeKindPointer(p) => match kind(p.ty) {
            TypeKindFunction(f) => Some(f),
            _ => None,
        },
        _ => None,
    }
}

pub fn int_kind(t: type_id) -> Option<IntKind> {
    match kind(t) {
        TypeKindInt(k) => Some(k),
        TypeKindBool => Some(IntKindUChar),
        TypeKindEnum(e) => Some(enum_info(e).underlying),
        _ => None,
    }
}

pub fn is_signed(t: type_id) -> bool {
    match int_kind(t) {
        Some(k) => int_signed(k),
        None => is_float(t),
    }
}

pub fn int_signed(k: IntKind) -> bool {
    matches!(
        k,
        IntKindChar
            | IntKindSChar
            | IntKindShort
            | IntKindInt
            | IntKindLong
            | IntKindLongLong
            | IntKindInt128
    )
}

pub fn int_size(k: IntKind) -> u64 {
    match k {
        IntKindChar | IntKindSChar | IntKindUChar => 1,
        IntKindShort | IntKindUShort => 2,
        IntKindInt | IntKindUInt => 4,
        IntKindLong | IntKindULong | IntKindLongLong | IntKindULongLong => 8,
        IntKindInt128 | IntKindUInt128 => 16,
    }
}

pub fn int_rank(k: IntKind) -> u8 {
    match k {
        IntKindChar | IntKindSChar | IntKindUChar => 1,
        IntKindShort | IntKindUShort => 2,
        IntKindInt | IntKindUInt => 3,
        IntKindLong | IntKindULong => 4,
        IntKindLongLong | IntKindULongLong => 5,
        IntKindInt128 | IntKindUInt128 => 6,
    }
}

pub fn to_unsigned(k: IntKind) -> IntKind {
    match k {
        IntKindChar | IntKindSChar => IntKindUChar,
        IntKindShort => IntKindUShort,
        IntKindInt => IntKindUInt,
        IntKindLong => IntKindULong,
        IntKindLongLong => IntKindULongLong,
        IntKindInt128 => IntKindUInt128,
        k => k,
    }
}

/// Whether the type has a drop obligation: a record with a drop function,
/// a record with an owned member, or an array of owned elements.
pub fn is_owned(t: type_id) -> bool {
    match kind(t) {
        TypeKindRecord(r) => with_record(r, |r| r.owned),
        TypeKindArray(e, _) => is_owned(e.ty),
        _ => false,
    }
}

/// Whether values of this type can hold a pointer or borrow, so the borrow
/// checker must track where they point.
pub fn may_hold_pointer(t: type_id) -> bool {
    match kind(t) {
        TypeKindPointer(_) | TypeKindRef { .. } | TypeKindSlice { .. } | TypeKindNullptr => true,
        TypeKindArray(e, _) => may_hold_pointer(e.ty),
        TypeKindRecord(r) => record(r).fields.iter().any(|f| may_hold_pointer(f.ty.ty)),
        _ => false,
    }
}

pub fn is_complete(t: type_id) -> bool {
    match kind(t) {
        TypeKindVoid | TypeKindFunction(_) => false,
        TypeKindArray(e, n) => n.is_some() && is_complete(e.ty),
        TypeKindRecord(r) => with_record(r, |r| r.complete),
        TypeKindEnum(e) => enum_info(e).complete || enum_info(e).fixed,
        _ => true,
    }
}

// ---- layout (x86-64 System V) ----

pub fn size_of(t: type_id) -> u64 {
    match kind(t) {
        TypeKindVoid => 1,
        TypeKindBool => 1,
        TypeKindInt(k) => int_size(k),
        TypeKindFloat(FloatKindFloat) => 4,
        TypeKindFloat(FloatKindDouble) => 8,
        TypeKindFloat(FloatKindLongDouble) => 16,
        TypeKindPointer(_) | TypeKindRef { .. } | TypeKindNullptr => 8,
        TypeKindSlice { .. } => 16,
        TypeKindArray(e, n) => size_of(e.ty) * n.unwrap_or(0),
        TypeKindFunction(_) => 1,
        TypeKindRecord(r) => with_record(r, |r| r.size),
        TypeKindEnum(e) => int_size(enum_info(e).underlying),
        TypeKindError => 1,
    }
}

pub fn align_of(t: type_id) -> u64 {
    match kind(t) {
        TypeKindArray(e, _) => align_of(e.ty),
        TypeKindRecord(r) => with_record(r, |r| r.align.max(1)),
        TypeKindSlice { .. } => 8,
        TypeKindFunction(_) | TypeKindVoid | TypeKindError => 1,
        _ => size_of(t),
    }
}

pub fn align_to(v: u64, a: u64) -> u64 {
    if a <= 1 { v } else { v.div_ceil(a) * a }
}

/// Lays out a record's fields in place. `fields` hold types, names, and
/// bit-field widths in declaration order.
pub fn layout_record(
    is_union: bool,
    fields: &mut [field],
    widths: &[Option<u32>],
    min_align: u64,
) -> (u64, u64) {
    let mut offset_bits: u64 = 0;
    let mut align: u64 = min_align.max(1);
    let mut size: u64 = 0;
    for (f, w) in fields.iter_mut().zip(widths) {
        let fsize = size_of(f.ty.ty);
        let falign = align_of(f.ty.ty);
        match w {
            Some(width) => {
                let unit_bits = fsize * 8;
                let width = *width as u64;
                if is_union {
                    f.offset = 0;
                    f.bits = Some(bit_field { shift: 0, width: width as u32 });
                    size = size.max(fsize);
                } else {
                    if width == 0 {
                        offset_bits = align_to(offset_bits, unit_bits);
                        f.offset = offset_bits / 8;
                        f.bits = Some(bit_field { shift: 0, width: 0 });
                        continue;
                    }
                    let unit_start = offset_bits / unit_bits * unit_bits;
                    if offset_bits + width > unit_start + unit_bits {
                        offset_bits = align_to(offset_bits, unit_bits);
                    }
                    let unit_start = offset_bits / unit_bits * unit_bits;
                    f.offset = unit_start / 8;
                    f.bits = Some(bit_field {
                        shift: (offset_bits - unit_start) as u32,
                        width: width as u32,
                    });
                    offset_bits += width;
                }
                if f.name.is_some() {
                    align = align.max(falign);
                }
            }
            None => {
                align = align.max(falign);
                if is_union {
                    f.offset = 0;
                    size = size.max(fsize);
                } else {
                    let off = align_to(offset_bits.div_ceil(8), falign);
                    f.offset = off;
                    offset_bits = (off + fsize) * 8;
                }
            }
        }
    }
    if !is_union {
        size = offset_bits.div_ceil(8);
    }
    (align_to(size, align), align)
}

// ---- conversions ----

pub fn promote(t: type_id) -> type_id {
    match kind(t) {
        TypeKindBool => int(),
        TypeKindInt(k) if int_rank(k) < 3 => int(),
        TypeKindEnum(e) => {
            let u = enum_info(e).underlying;
            if int_rank(u) < 3 { int() } else { int_of(u) }
        }
        _ => t,
    }
}

/// The usual arithmetic conversions (C23 6.3.1.8).
pub fn usual_arith(a: type_id, b: type_id) -> type_id {
    match (kind(a), kind(b)) {
        (TypeKindFloat(x), TypeKindFloat(y)) => {
            let rank = |k: FloatKind| match k {
                FloatKindFloat => 0,
                FloatKindDouble => 1,
                FloatKindLongDouble => 2,
            };
            return if rank(x) >= rank(y) { a } else { b };
        }
        (TypeKindFloat(_), _) => return a,
        (_, TypeKindFloat(_)) => return b,
        _ => {}
    }
    let a = promote(a);
    let b = promote(b);
    let (ka, kb) = (int_kind(a).unwrap_or(IntKindInt), int_kind(b).unwrap_or(IntKindInt));
    if ka == kb {
        return a;
    }
    let (sa, sb) = (int_signed(ka), int_signed(kb));
    if sa == sb {
        return if int_rank(ka) >= int_rank(kb) { a } else { b };
    }
    let (s, u) = if sa { (ka, kb) } else { (kb, ka) };
    if int_rank(u) >= int_rank(s) {
        return int_of(u);
    }
    if int_size(s) > int_size(u) {
        return int_of(s);
    }
    int_of(to_unsigned(s))
}

/// Whether two types are compatible (C23 6.2.7). Lifetimes are ignored.
pub fn compatible(a: qual_type, b: qual_type) -> bool {
    if a.quals & !QualRestrict != b.quals & !QualRestrict {
        return false;
    }
    compatible_unqual(a.ty, b.ty)
}

pub fn compatible_unqual(a: type_id, b: type_id) -> bool {
    if a == b {
        return true;
    }
    match (kind(a), kind(b)) {
        (TypeKindPointer(x), TypeKindPointer(y)) => compatible(x, y),
        (TypeKindArray(x, n), TypeKindArray(y, m)) => {
            compatible(x, y) && (n.is_none() || m.is_none() || n == m)
        }
        (TypeKindFunction(f), TypeKindFunction(g)) => {
            if !compatible(f.ret, g.ret)
                || f.variadic != g.variadic
                || f.params.len() != g.params.len()
            {
                return false;
            }
            f.params.iter().zip(&g.params).all(|(x, y)| compatible(x.unqual(), y.unqual()))
        }
        (TypeKindEnum(e), TypeKindInt(k)) | (TypeKindInt(k), TypeKindEnum(e)) => {
            enum_info(e).underlying == k
        }
        (
            TypeKindRef { mutable: m1, pointee: p1, .. },
            TypeKindRef { mutable: m2, pointee: p2, .. },
        ) => m1 == m2 && compatible(p1, p2),
        (
            TypeKindSlice { mutable: m1, elem: e1, .. },
            TypeKindSlice { mutable: m2, elem: e2, .. },
        ) => m1 == m2 && compatible(e1, e2),
        (TypeKindError, _) | (_, TypeKindError) => true,
        _ => false,
    }
}

/// Removes lifetime names, so signatures can be compared structurally.
pub fn strip_lifetimes(t: qual_type) -> qual_type {
    let k = match kind(t.ty) {
        TypeKindRef { mutable, pointee, .. } => {
            TypeKindRef { mutable, lifetime: None, pointee: strip_lifetimes(pointee) }
        }
        TypeKindSlice { mutable, elem, .. } => {
            TypeKindSlice { mutable, lifetime: None, elem: strip_lifetimes(elem) }
        }
        TypeKindPointer(p) => TypeKindPointer(strip_lifetimes(p)),
        TypeKindFunction(f) => TypeKindFunction(func_type {
            ret: strip_lifetimes(f.ret),
            params: f.params.iter().map(|p| strip_lifetimes(*p)).collect(),
            variadic: f.variadic,
        }),
        _ => return t,
    };
    qual_type { ty: intern(k), quals: t.quals }
}

// ---- printing ----

fn quals_str(q: u8) -> String {
    let mut s = String::new();
    if q & QualConst != 0 {
        s.push_str("const ");
    }
    if q & QualVolatile != 0 {
        s.push_str("volatile ");
    }
    if q & QualRestrict != 0 {
        s.push_str("restrict ");
    }
    if q & QualAtomic != 0 {
        s.push_str("_Atomic ");
    }
    s
}

pub fn int_name(k: IntKind) -> &'static str {
    match k {
        IntKindChar => "char",
        IntKindSChar => "signed char",
        IntKindUChar => "unsigned char",
        IntKindShort => "short",
        IntKindUShort => "unsigned short",
        IntKindInt => "int",
        IntKindUInt => "unsigned int",
        IntKindLong => "long",
        IntKindULong => "unsigned long",
        IntKindLongLong => "long long",
        IntKindULongLong => "unsigned long long",
        IntKindInt128 => "__int128",
        IntKindUInt128 => "unsigned __int128",
    }
}

/// Spells a type the way a declaration would, around `inner` (the declarator).
pub fn display_decl(t: qual_type, inner: &str) -> String {
    let q = quals_str(t.quals);
    match kind(t.ty) {
        TypeKindPointer(p) => {
            let mut s = String::from("*");
            if t.quals != 0 {
                s.push_str(&format!(" {}", q.trim_end()));
                if !inner.is_empty() {
                    s.push(' ');
                }
            }
            s.push_str(inner);
            let needs_paren = matches!(kind(p.ty), TypeKindArray(..) | TypeKindFunction(_));
            let s = if needs_paren { format!("({s})") } else { s };
            display_decl(p, &s)
        }
        TypeKindArray(e, n) => {
            let s = match n {
                Some(n) => format!("{inner}[{n}]"),
                None => format!("{inner}[]"),
            };
            display_decl(e, &s)
        }
        TypeKindFunction(f) => {
            let mut ps: Vec<String> = f.params.iter().map(|p| display(*p)).collect();
            if f.variadic {
                ps.push("...".into());
            }
            if ps.is_empty() {
                ps.push("void".into());
            }
            display_decl(f.ret, &format!("{inner}({})", ps.join(", ")))
        }
        _ => {
            let base = format!("{q}{}", base_name(t.ty));
            if inner.is_empty() { base } else { format!("{base} {inner}") }
        }
    }
}

fn lifetime_str(l: Option<sym>) -> String {
    l.map(|l| format!("{l} ")).unwrap_or_default()
}

fn base_name(t: type_id) -> String {
    match kind(t) {
        TypeKindVoid => "void".into(),
        TypeKindBool => "bool".into(),
        TypeKindInt(k) => int_name(k).into(),
        TypeKindFloat(FloatKindFloat) => "float".into(),
        TypeKindFloat(FloatKindDouble) => "double".into(),
        TypeKindFloat(FloatKindLongDouble) => "long double".into(),
        TypeKindRecord(r) => {
            let r = record(r);
            let kw = if r.is_union { "union" } else { "struct" };
            match r.tag {
                Some(t) => format!("{kw} {t}"),
                None => format!("{kw} (anonymous)"),
            }
        }
        TypeKindEnum(e) => match enum_info(e).tag {
            Some(t) => format!("enum {t}"),
            None => "enum (anonymous)".into(),
        },
        TypeKindRef { mutable, lifetime, pointee } => {
            let m = if mutable { "mut " } else { "" };
            let inner = display(pointee);
            let inner = if matches!(
                kind(pointee.ty),
                TypeKindPointer(_) | TypeKindArray(..) | TypeKindFunction(_)
            ) {
                format!("({inner})")
            } else {
                inner
            };
            format!("ref {}{m}{inner}", lifetime_str(lifetime))
        }
        TypeKindSlice { mutable, lifetime, elem } => {
            let m = if mutable { "mut " } else { "" };
            format!("ref {}{m}[{}]", lifetime_str(lifetime), display(elem))
        }
        TypeKindNullptr => "nullptr_t".into(),
        TypeKindError => "<error>".into(),
        _ => "?".into(),
    }
}

pub fn display(t: qual_type) -> String {
    display_decl(t, "")
}

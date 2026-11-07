/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// `.crmi` module interface files (docs/MODULES.md, section 6; D-030).
//
// Layout: the bytes `CRMI`, a little-endian u32 format version, a
// little-endian u64 interface hash, then the payload. The hash is FNV-1a
// over the payload, which holds only the exports and the types they need,
// so editing a function body leaves it unchanged.
use super::*;
use crate::sema::tir::{ConstValueFloat, ConstValueInt};
use crate::sema::types::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

pub const FormatVersion: u32 = 1;

thread_local! {
    /// Records and enums by (module, tag), so one type imported through
    /// several interfaces stays one type.
    static Records: RefCell<HashMap<(String, String), u32>> = RefCell::new(HashMap::new());
    static Enums: RefCell<HashMap<(String, String), u32>> = RefCell::new(HashMap::new());
}

/// Registers a record defined in this process, under its identity key.
pub fn register_record(module: Option<sym>, tag: sym, id: u32) {
    let key = (module.map(|m| m.to_string()).unwrap_or_default(), tag.to_string());
    Records.with(|r| {
        r.borrow_mut().entry(key).or_insert(id);
    });
}

pub fn lookup_record(module: Option<sym>, tag: sym) -> Option<u32> {
    let key = (module.map(|m| m.to_string()).unwrap_or_default(), tag.to_string());
    Records.with(|r| r.borrow().get(&key).copied())
}

pub fn register_enum(module: Option<sym>, tag: sym, id: u32) {
    let key = (module.map(|m| m.to_string()).unwrap_or_default(), tag.to_string());
    Enums.with(|r| {
        r.borrow_mut().entry(key).or_insert(id);
    });
}

fn fnv1a(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for x in b {
        h ^= *x as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn build(name: sym, exports: Vec<export_item>) -> module_interface {
    let mut i = module_interface { name, hash: 0, exports };
    let payload = encode_payload(&i);
    i.hash = fnv1a(&payload);
    i
}

// ---- encoding ----

struct writer {
    types: Vec<Vec<u8>>,
    type_ids: HashMap<type_id, u32>,
    records: Vec<Vec<u8>>,
    record_ids: HashMap<u32, u32>,
    enums: Vec<Vec<u8>>,
    enum_ids: HashMap<u32, u32>,
}

fn put_u8(o: &mut Vec<u8>, v: u8) {
    o.push(v);
}
fn put_u32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(o: &mut Vec<u8>, v: u64) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn put_str(o: &mut Vec<u8>, s: &str) {
    put_u32(o, s.len() as u32);
    o.extend_from_slice(s.as_bytes());
}
fn put_opt_str(o: &mut Vec<u8>, s: Option<&str>) {
    match s {
        Some(s) => {
            put_u8(o, 1);
            put_str(o, s);
        }
        None => put_u8(o, 0),
    }
}

fn int_code(k: IntKind) -> u8 {
    match k {
        IntKindChar => 0,
        IntKindSChar => 1,
        IntKindUChar => 2,
        IntKindShort => 3,
        IntKindUShort => 4,
        IntKindInt => 5,
        IntKindUInt => 6,
        IntKindLong => 7,
        IntKindULong => 8,
        IntKindLongLong => 9,
        IntKindULongLong => 10,
        IntKindInt128 => 11,
        IntKindUInt128 => 12,
    }
}

fn int_from(c: u8) -> Result<IntKind, String> {
    Ok(match c {
        0 => IntKindChar,
        1 => IntKindSChar,
        2 => IntKindUChar,
        3 => IntKindShort,
        4 => IntKindUShort,
        5 => IntKindInt,
        6 => IntKindUInt,
        7 => IntKindLong,
        8 => IntKindULong,
        9 => IntKindLongLong,
        10 => IntKindULongLong,
        11 => IntKindInt128,
        12 => IntKindUInt128,
        _ => return Err(format!("bad integer kind {c}")),
    })
}

impl writer {
    fn qt(&mut self, o: &mut Vec<u8>, t: qual_type) {
        let id = self.ty(t.ty);
        put_u32(o, id);
        put_u8(o, t.quals);
    }

    fn ty(&mut self, t: type_id) -> u32 {
        if let Some(&i) = self.type_ids.get(&t) {
            return i;
        }
        let idx = self.types.len() as u32;
        self.type_ids.insert(t, idx);
        self.types.push(Vec::new());
        let mut o = Vec::new();
        match kind(t) {
            TypeKindVoid => put_u8(&mut o, 0),
            TypeKindBool => put_u8(&mut o, 1),
            TypeKindInt(k) => {
                put_u8(&mut o, 2);
                put_u8(&mut o, int_code(k));
            }
            TypeKindFloat(k) => {
                put_u8(&mut o, 3);
                put_u8(
                    &mut o,
                    match k {
                        FloatKindFloat => 0,
                        FloatKindDouble => 1,
                        FloatKindLongDouble => 2,
                    },
                );
            }
            TypeKindPointer(p) => {
                put_u8(&mut o, 4);
                self.qt(&mut o, p);
            }
            TypeKindArray(e, n) => {
                put_u8(&mut o, 5);
                self.qt(&mut o, e);
                put_u8(&mut o, n.is_some() as u8);
                put_u64(&mut o, n.unwrap_or(0));
            }
            TypeKindFunction(f) => {
                put_u8(&mut o, 6);
                self.qt(&mut o, f.ret);
                put_u32(&mut o, f.params.len() as u32);
                for p in &f.params {
                    self.qt(&mut o, *p);
                }
                put_u8(&mut o, f.variadic as u8);
            }
            TypeKindRecord(r) => {
                put_u8(&mut o, 7);
                let ri = self.record(r);
                put_u32(&mut o, ri);
            }
            TypeKindEnum(e) => {
                put_u8(&mut o, 8);
                let ei = self.enum_(e);
                put_u32(&mut o, ei);
            }
            TypeKindRef { mutable, lifetime, pointee } => {
                put_u8(&mut o, 9);
                put_u8(&mut o, mutable as u8);
                put_opt_str(&mut o, lifetime.map(|l| l.as_str()));
                self.qt(&mut o, pointee);
            }
            TypeKindSlice { mutable, lifetime, elem } => {
                put_u8(&mut o, 10);
                put_u8(&mut o, mutable as u8);
                put_opt_str(&mut o, lifetime.map(|l| l.as_str()));
                self.qt(&mut o, elem);
            }
            TypeKindNullptr => put_u8(&mut o, 11),
            TypeKindError => put_u8(&mut o, 12),
        }
        self.types[idx as usize] = o;
        idx
    }

    fn record(&mut self, r: u32) -> u32 {
        if let Some(&i) = self.record_ids.get(&r) {
            return i;
        }
        let idx = self.records.len() as u32;
        self.record_ids.insert(r, idx);
        self.records.push(Vec::new());
        let rd = record(r);
        let mut o = Vec::new();
        put_str(&mut o, &rd.module.map(|m| m.to_string()).unwrap_or_default());
        put_opt_str(&mut o, rd.tag.map(|t| t.as_str()));
        put_u8(&mut o, rd.is_union as u8);
        put_u8(&mut o, rd.complete as u8);
        put_u64(&mut o, rd.size);
        put_u64(&mut o, rd.align);
        put_u8(&mut o, rd.owned as u8);
        match &rd.drop {
            Some(d) => {
                put_u8(&mut o, 1);
                put_str(&mut o, &d.symbol);
                put_str(&mut o, d.name.as_str());
            }
            None => put_u8(&mut o, 0),
        }
        put_u32(&mut o, rd.fields.len() as u32);
        for f in &rd.fields {
            put_opt_str(&mut o, f.name.map(|n| n.as_str()));
            self.qt(&mut o, f.ty);
            put_u64(&mut o, f.offset);
            match &f.bits {
                Some(b) => {
                    put_u8(&mut o, 1);
                    put_u32(&mut o, b.shift);
                    put_u32(&mut o, b.width);
                }
                None => put_u8(&mut o, 0),
            }
        }
        self.records[idx as usize] = o;
        idx
    }

    fn enum_(&mut self, e: u32) -> u32 {
        if let Some(&i) = self.enum_ids.get(&e) {
            return i;
        }
        let idx = self.enums.len() as u32;
        self.enum_ids.insert(e, idx);
        let ed = enum_info(e);
        let mut o = Vec::new();
        put_str(&mut o, &ed.module.map(|m| m.to_string()).unwrap_or_default());
        put_opt_str(&mut o, ed.tag.map(|t| t.as_str()));
        put_u8(&mut o, int_code(ed.underlying));
        put_u8(&mut o, ed.fixed as u8);
        put_u8(&mut o, ed.complete as u8);
        put_u32(&mut o, ed.enumerators.len() as u32);
        for (n, v) in &ed.enumerators {
            put_str(&mut o, n.as_str());
            o.extend_from_slice(&v.to_le_bytes());
        }
        self.enums.push(o);
        idx
    }
}

fn encode_payload(i: &module_interface) -> Vec<u8> {
    let mut w = writer {
        types: vec![],
        type_ids: HashMap::new(),
        records: vec![],
        record_ids: HashMap::new(),
        enums: vec![],
        enum_ids: HashMap::new(),
    };
    let mut ex = Vec::new();
    put_u32(&mut ex, i.exports.len() as u32);
    for e in &i.exports {
        put_str(&mut ex, e.name.as_str());
        match &e.kind {
            ExportKindFunc { symbol, ty, unsafe_fn, safe, c_abi, noreturn } => {
                put_u8(&mut ex, 0);
                put_str(&mut ex, symbol);
                w.qt(&mut ex, *ty);
                for b in [*unsafe_fn, *safe, *c_abi, *noreturn] {
                    put_u8(&mut ex, b as u8);
                }
            }
            ExportKindVar { symbol, ty, needs_unsafe, c_abi } => {
                put_u8(&mut ex, 1);
                put_str(&mut ex, symbol);
                w.qt(&mut ex, *ty);
                put_u8(&mut ex, *needs_unsafe as u8);
                put_u8(&mut ex, *c_abi as u8);
            }
            ExportKindConst { ty, value } => {
                put_u8(&mut ex, 2);
                w.qt(&mut ex, *ty);
                match value {
                    ConstValueInt(v) => {
                        put_u8(&mut ex, 0);
                        ex.extend_from_slice(&v.to_le_bytes());
                    }
                    ConstValueFloat(f) => {
                        put_u8(&mut ex, 1);
                        put_u64(&mut ex, f.to_bits());
                    }
                }
            }
            ExportKindTypedef(t) => {
                put_u8(&mut ex, 3);
                w.qt(&mut ex, *t);
            }
            ExportKindRecord(r) => {
                put_u8(&mut ex, 4);
                let ri = w.record(*r);
                put_u32(&mut ex, ri);
            }
            ExportKindEnum(e) => {
                put_u8(&mut ex, 5);
                let ei = w.enum_(*e);
                put_u32(&mut ex, ei);
            }
        }
    }
    let mut o = Vec::new();
    put_str(&mut o, i.name.as_str());
    for table in [&w.types, &w.records, &w.enums] {
        put_u32(&mut o, table.len() as u32);
        for t in table.iter() {
            put_u32(&mut o, t.len() as u32);
            o.extend_from_slice(t);
        }
    }
    o.extend_from_slice(&ex);
    o
}

pub fn encode(i: &module_interface) -> Vec<u8> {
    let payload = encode_payload(i);
    let mut o = b"CRMI".to_vec();
    put_u32(&mut o, FormatVersion);
    put_u64(&mut o, fnv1a(&payload));
    o.extend_from_slice(&payload);
    o
}

// ---- decoding ----

struct reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.pos + n > self.b.len() {
            return Err("truncated interface".into());
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i128(&mut self) -> Result<i128, String> {
        Ok(i128::from_le_bytes(self.take(16)?.try_into().unwrap()))
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn opt_str(&mut self) -> Result<Option<String>, String> {
        if self.u8()? == 1 { Ok(Some(self.str()?)) } else { Ok(None) }
    }
}

struct decoder {
    types: Vec<Vec<u8>>,
    records: Vec<Vec<u8>>,
    enums: Vec<Vec<u8>>,
    type_memo: HashMap<u32, type_id>,
    record_memo: HashMap<u32, u32>,
    enum_memo: HashMap<u32, u32>,
}

impl decoder {
    fn qt(&mut self, r: &mut reader) -> Result<qual_type, String> {
        let t = r.u32()?;
        let quals = r.u8()?;
        Ok(qual_type { ty: self.ty(t)?, quals })
    }

    fn ty(&mut self, i: u32) -> Result<type_id, String> {
        if let Some(t) = self.type_memo.get(&i) {
            return Ok(*t);
        }
        let bytes = self.types.get(i as usize).ok_or("bad type index")?.clone();
        let mut r = reader { b: &bytes, pos: 0 };
        let k = match r.u8()? {
            0 => TypeKindVoid,
            1 => TypeKindBool,
            2 => TypeKindInt(int_from(r.u8()?)?),
            3 => TypeKindFloat(match r.u8()? {
                0 => FloatKindFloat,
                1 => FloatKindDouble,
                _ => FloatKindLongDouble,
            }),
            4 => TypeKindPointer(self.qt(&mut r)?),
            5 => {
                let e = self.qt(&mut r)?;
                let has = r.u8()? == 1;
                let n = r.u64()?;
                TypeKindArray(e, has.then_some(n))
            }
            6 => {
                let ret = self.qt(&mut r)?;
                let n = r.u32()?;
                let mut params = Vec::new();
                for _ in 0..n {
                    params.push(self.qt(&mut r)?);
                }
                let variadic = r.u8()? == 1;
                TypeKindFunction(func_type { ret, params, variadic })
            }
            7 => TypeKindRecord(self.record(r.u32()?)?),
            8 => TypeKindEnum(self.enum_(r.u32()?)?),
            9 | 10 => {
                let slice = bytes[0] == 10;
                let mutable = r.u8()? == 1;
                let lifetime = r.opt_str()?.map(|s| sym::intern(&s));
                let inner = self.qt(&mut r)?;
                if slice {
                    TypeKindSlice { mutable, lifetime, elem: inner }
                } else {
                    TypeKindRef { mutable, lifetime, pointee: inner }
                }
            }
            11 => TypeKindNullptr,
            _ => TypeKindError,
        };
        let t = intern(k);
        self.type_memo.insert(i, t);
        Ok(t)
    }

    fn record(&mut self, i: u32) -> Result<u32, String> {
        if let Some(r) = self.record_memo.get(&i) {
            return Ok(*r);
        }
        let bytes = self.records.get(i as usize).ok_or("bad record index")?.clone();
        let mut r = reader { b: &bytes, pos: 0 };
        let module = r.str()?;
        let tag = r.opt_str()?;
        let is_union = r.u8()? == 1;
        let complete = r.u8()? == 1;
        let size = r.u64()?;
        let align = r.u64()?;
        let owned = r.u8()? == 1;
        let drop = if r.u8()? == 1 {
            let symbol = r.str()?;
            let name = sym::intern(&r.str()?);
            Some(drop_info { symbol, name })
        } else {
            None
        };
        if let Some(t) = &tag {
            let existing = Records.with(|m| m.borrow().get(&(module.clone(), t.clone())).copied());
            if let Some(id) = existing {
                if with_record(id, |rd| rd.complete) || !complete {
                    self.record_memo.insert(i, id);
                    return Ok(id);
                }
            }
        }
        let modsym = if module.is_empty() { None } else { Some(sym::intern(&module)) };
        let id = match tag
            .as_ref()
            .and_then(|t| Records.with(|m| m.borrow().get(&(module.clone(), t.clone())).copied()))
        {
            Some(id) => id,
            None => new_record(record_def {
                is_union,
                tag: tag.as_ref().map(|t| sym::intern(t)),
                module: modsym,
                complete: false,
                fields: vec![],
                size: 0,
                align: 1,
                drop: None,
                owned: false,
                span: crate::diag::span::dummy(),
            }),
        };
        if let Some(t) = &tag {
            Records.with(|m| m.borrow_mut().insert((module.clone(), t.clone()), id));
        }
        self.record_memo.insert(i, id);
        let n = r.u32()?;
        let mut fields = Vec::new();
        for _ in 0..n {
            let name = r.opt_str()?.map(|s| sym::intern(&s));
            let ty = self.qt(&mut r)?;
            let offset = r.u64()?;
            let bits = if r.u8()? == 1 {
                Some(bit_field { shift: r.u32()?, width: r.u32()? })
            } else {
                None
            };
            fields.push(field { name, ty, offset, bits, span: crate::diag::span::dummy() });
        }
        update_record(id, |rd| {
            rd.fields = fields;
            rd.size = size;
            rd.align = align;
            rd.complete = complete;
            rd.owned = owned;
            rd.drop = drop;
        });
        Ok(id)
    }

    fn enum_(&mut self, i: u32) -> Result<u32, String> {
        if let Some(e) = self.enum_memo.get(&i) {
            return Ok(*e);
        }
        let bytes = self.enums.get(i as usize).ok_or("bad enum index")?.clone();
        let mut r = reader { b: &bytes, pos: 0 };
        let module = r.str()?;
        let tag = r.opt_str()?;
        let underlying = int_from(r.u8()?)?;
        let fixed = r.u8()? == 1;
        let complete = r.u8()? == 1;
        let n = r.u32()?;
        let mut enumerators = Vec::new();
        for _ in 0..n {
            let name = sym::intern(&r.str()?);
            enumerators.push((name, r.i128()?));
        }
        if let Some(t) = &tag {
            if let Some(id) = Enums.with(|m| m.borrow().get(&(module.clone(), t.clone())).copied())
            {
                self.enum_memo.insert(i, id);
                return Ok(id);
            }
        }
        let id = new_enum(enum_def {
            tag: tag.as_ref().map(|t| sym::intern(t)),
            module: if module.is_empty() { None } else { Some(sym::intern(&module)) },
            complete,
            underlying,
            fixed,
            enumerators,
            span: crate::diag::span::dummy(),
        });
        if let Some(t) = tag {
            Enums.with(|m| m.borrow_mut().insert((module, t), id));
        }
        self.enum_memo.insert(i, id);
        Ok(id)
    }
}

pub fn decode(b: &[u8]) -> Result<module_interface, String> {
    if b.len() < 16 || &b[..4] != b"CRMI" {
        return Err("not a Crust interface file".into());
    }
    let mut r = reader { b, pos: 4 };
    let v = r.u32()?;
    if v != FormatVersion {
        return Err(format!(
            "interface format version {v} is not supported (this compiler reads version {FormatVersion})"
        ));
    }
    let hash = r.u64()?;
    if fnv1a(&b[16..]) != hash {
        return Err("interface hash does not match its contents".into());
    }
    let name = sym::intern(&r.str()?);
    let mut tables: Vec<Vec<Vec<u8>>> = Vec::new();
    for _ in 0..3 {
        let n = r.u32()?;
        let mut t = Vec::new();
        for _ in 0..n {
            let len = r.u32()? as usize;
            t.push(r.take(len)?.to_vec());
        }
        tables.push(t);
    }
    let enums = tables.pop().unwrap();
    let records = tables.pop().unwrap();
    let types = tables.pop().unwrap();
    let mut d = decoder {
        types,
        records,
        enums,
        type_memo: HashMap::new(),
        record_memo: HashMap::new(),
        enum_memo: HashMap::new(),
    };
    let n = r.u32()?;
    let mut exports = Vec::new();
    for _ in 0..n {
        let ename = sym::intern(&r.str()?);
        let kind = match r.u8()? {
            0 => {
                let symbol = r.str()?;
                let ty = d.qt(&mut r)?;
                let (u, s, c, nr) = (r.u8()? == 1, r.u8()? == 1, r.u8()? == 1, r.u8()? == 1);
                ExportKindFunc { symbol, ty, unsafe_fn: u, safe: s, c_abi: c, noreturn: nr }
            }
            1 => {
                let symbol = r.str()?;
                let ty = d.qt(&mut r)?;
                ExportKindVar { symbol, ty, needs_unsafe: r.u8()? == 1, c_abi: r.u8()? == 1 }
            }
            2 => {
                let ty = d.qt(&mut r)?;
                let value = if r.u8()? == 0 {
                    ConstValueInt(r.i128()?)
                } else {
                    ConstValueFloat(f64::from_bits(r.u64()?))
                };
                ExportKindConst { ty, value }
            }
            3 => ExportKindTypedef(d.qt(&mut r)?),
            4 => ExportKindRecord(d.record(r.u32()?)?),
            5 => ExportKindEnum(d.enum_(r.u32()?)?),
            k => return Err(format!("bad export kind {k}")),
        };
        exports.push(export_item { name: ename, kind });
    }
    Ok(module_interface { name, hash, exports })
}

/// The readable form printed by `crust --print-interface`.
pub fn print(i: &module_interface) -> String {
    let mut s =
        format!("module {}\nformat: {FormatVersion}\nhash: {:016x}\nexports:\n", i.name, i.hash);
    for e in &i.exports {
        let line = match &e.kind {
            ExportKindFunc { symbol, ty, unsafe_fn, safe, c_abi, .. } => {
                let mut attrs = String::new();
                if *c_abi {
                    attrs.push_str("[[crust::c_abi]] ");
                }
                if *safe {
                    attrs.push_str("[[crust::safe]] ");
                }
                let u = if *unsafe_fn { "unsafe " } else { "" };
                format!("{attrs}{u}{};  // symbol {symbol}", display_decl(*ty, e.name.as_str()))
            }
            ExportKindVar { symbol, ty, c_abi, .. } => {
                let a = if *c_abi { "[[crust::c_abi]] " } else { "" };
                format!("{a}{};  // symbol {symbol}", display_decl(*ty, e.name.as_str()))
            }
            ExportKindConst { ty, value } => {
                let v = match value {
                    ConstValueInt(v) => v.to_string(),
                    ConstValueFloat(f) => format!("{f:?}"),
                };
                format!("constexpr {} = {v};", display_decl(*ty, e.name.as_str()))
            }
            ExportKindTypedef(t) => format!("typedef {};", display_decl(*t, e.name.as_str())),
            ExportKindRecord(r) => {
                let rd = record(*r);
                let kw = if rd.is_union { "union" } else { "struct" };
                let mut b = format!("{kw} {} {{", e.name);
                for f in &rd.fields {
                    let n = f.name.map(|n| n.to_string()).unwrap_or_default();
                    b.push_str(&format!(" {};", display_decl(f.ty, &n)));
                }
                let d = rd.drop.map(|d| format!(" drop {}", d.symbol)).unwrap_or_default();
                b.push_str(&format!(" }};  // size {}, align {}{d}", rd.size, rd.align));
                b
            }
            ExportKindEnum(en) => {
                let ed = enum_info(*en);
                let vals: Vec<String> =
                    ed.enumerators.iter().map(|(n, v)| format!("{n} = {v}")).collect();
                format!("enum {} : {} {{ {} }};", e.name, int_name(ed.underlying), vals.join(", "))
            }
        };
        s.push_str("  ");
        s.push_str(&line);
        s.push('\n');
    }
    s
}

pub fn print_file(p: &Path) -> Result<String, String> {
    let b = std::fs::read(p).map_err(|e| format!("cannot read `{}`: {e}", p.display()))?;
    let i = decode(&b).map_err(|e| format!("`{}`: {e}", p.display()))?;
    Ok(print(&i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensure::ensure;

    #[test]
    fn round_trips_and_keeps_hash() {
        let pt = new_record(record_def {
            is_union: false,
            tag: Some(sym::intern("rt_point")),
            module: Some(sym::intern("rt")),
            complete: true,
            fields: vec![field {
                name: Some(sym::intern("x")),
                ty: q(int()),
                offset: 0,
                bits: None,
                span: crate::diag::span::dummy(),
            }],
            size: 4,
            align: 4,
            drop: None,
            owned: false,
            span: crate::diag::span::dummy(),
        });
        let fty = intern(TypeKindFunction(func_type {
            ret: q(int()),
            params: vec![q(intern(TypeKindRecord(pt)))],
            variadic: false,
        }));
        let i = build(
            sym::intern("rt"),
            vec![
                export_item { name: sym::intern("rt_point"), kind: ExportKindRecord(pt) },
                export_item {
                    name: sym::intern("get"),
                    kind: ExportKindFunc {
                        symbol: "_CR.rt.get".into(),
                        ty: q(fty),
                        unsafe_fn: false,
                        safe: false,
                        c_abi: false,
                        noreturn: false,
                    },
                },
            ],
        );
        let bytes = encode(&i);
        let d = ensure(decode(&bytes)).is_ok().into_value();
        ensure(d.hash).named("hash after a round trip").is(i.hash);
        ensure(d.exports.len()).named("exports after a round trip").is(2);
        ensure(print(&d)).contains("struct rt_point { int x; }");
    }

    #[test]
    fn rejects_corrupt_files() {
        // The interface type has no Debug, so look at the error side.
        ensure(decode(b"CRMI\x01\0\0\0\0\0\0\0\0\0\0\0xx").err()).is_some();
        ensure(decode(b"nope").err()).is_some();
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Argument and return classification for the x86-64 System V psABI
// (docs/ABI.md, section 1), matching what clang emits for the same C types.
use crate::sema::types::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind {
    /// Not passed at all (an empty struct).
    ArgKindIgnore,
    /// One scalar LLVM value, with an optional extension attribute.
    ArgKindDirect(String, Option<&'static str>),
    /// Split into scalar pieces: (LLVM type, byte offset, byte size).
    ArgKindExpand(Vec<(String, u64, u64)>),
    /// In memory: `byval` for parameters, `sret` for returns.
    ArgKindMemory,
}
pub use ArgKind::*;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Class {
    ClassNone,
    ClassInteger,
    ClassSse,
    ClassMemory,
}
use Class::*;

/// The LLVM type of a scalar Crust type as stored in memory.
pub fn llvm_scalar(t: type_id) -> &'static str {
    match kind(t) {
        TypeKindBool => "i8",
        TypeKindInt(k) => match int_size(k) {
            1 => "i8",
            2 => "i16",
            4 => "i32",
            8 => "i64",
            _ => "i128",
        },
        TypeKindEnum(e) => match int_size(enum_info(e).underlying) {
            1 => "i8",
            2 => "i16",
            4 => "i32",
            _ => "i64",
        },
        TypeKindFloat(FloatKindFloat) => "float",
        TypeKindFloat(FloatKindDouble) => "double",
        TypeKindFloat(FloatKindLongDouble) => "x86_fp80",
        TypeKindPointer(_) | TypeKindRef { .. } | TypeKindNullptr | TypeKindFunction(_) => "ptr",
        TypeKindVoid => "void",
        _ => "ptr",
    }
}

/// Whether values of this type live in memory rather than in one register.
pub fn is_aggregate(t: type_id) -> bool {
    matches!(kind(t), TypeKindRecord(_) | TypeKindArray(..) | TypeKindSlice { .. })
}

fn leaves(t: type_id, base: u64, out: &mut Vec<(u64, type_id)>) {
    match kind(t) {
        TypeKindRecord(r) => {
            let rd = record(r);
            for f in &rd.fields {
                leaves(f.ty.ty, base + f.offset, out);
            }
        }
        TypeKindArray(e, Some(n)) => {
            let sz = size_of(e.ty);
            for i in 0..n {
                leaves(e.ty, base + i * sz, out);
            }
        }
        TypeKindSlice { .. } => {
            out.push((base, size_t()));
            out.push((base + 8, size_t()));
        }
        TypeKindArray(_, None) => {}
        _ => out.push((base, t)),
    }
}

pub fn classify(t: type_id) -> ArgKind {
    if !is_aggregate(t) {
        if is_void(t) {
            return ArgKindIgnore;
        }
        let ext = match kind(t) {
            TypeKindBool => Some("zeroext"),
            TypeKindInt(_) | TypeKindEnum(_) if int_size(int_kind(t).unwrap()) < 4 => {
                if is_signed(t) { Some("signext") } else { Some("zeroext") }
            }
            _ => None,
        };
        return ArgKindDirect(llvm_scalar(t).to_string(), ext);
    }
    let size = size_of(t);
    if size == 0 {
        return ArgKindIgnore;
    }
    if size > 16 {
        return ArgKindMemory;
    }
    let mut ls = Vec::new();
    leaves(t, 0, &mut ls);
    let n = size.div_ceil(8) as usize;
    let mut classes = vec![ClassNone; n];
    for (off, lt) in &ls {
        let c = match kind(*lt) {
            TypeKindFloat(FloatKindLongDouble) => ClassMemory,
            TypeKindFloat(_) => ClassSse,
            _ => ClassInteger,
        };
        let lsz = size_of(*lt).max(1);
        if off % lsz.min(8) != 0 {
            return ArgKindMemory;
        }
        let i = (*off / 8) as usize;
        if i >= n {
            continue;
        }
        classes[i] = match (classes[i], c) {
            (ClassMemory, _) | (_, ClassMemory) => ClassMemory,
            (ClassInteger, _) | (_, ClassInteger) => ClassInteger,
            _ => ClassSse,
        };
    }
    if classes.contains(&ClassMemory) {
        return ArgKindMemory;
    }
    let mut pieces = Vec::new();
    for (i, c) in classes.iter().enumerate() {
        let off = i as u64 * 8;
        let bytes = (size - off).min(8);
        let ty = match c {
            ClassSse => {
                let floats: Vec<&(u64, type_id)> = ls
                    .iter()
                    .filter(|(o, lt)| {
                        *o >= off
                            && *o < off + 8
                            && matches!(kind(*lt), TypeKindFloat(FloatKindFloat))
                    })
                    .collect();
                let doubles = ls.iter().any(|(o, lt)| {
                    *o >= off && *o < off + 8 && matches!(kind(*lt), TypeKindFloat(FloatKindDouble))
                });
                if doubles {
                    "double".to_string()
                } else if floats.len() >= 2 {
                    "<2 x float>".to_string()
                } else if bytes <= 4 || floats.iter().all(|f| f.0 == off) && bytes <= 4 {
                    "float".to_string()
                } else if floats.len() == 1 && floats[0].0 == off + 4 {
                    "<2 x float>".to_string()
                } else {
                    "double".to_string()
                }
            }
            _ => format!("i{}", bytes * 8),
        };
        let psize = match ty.as_str() {
            "double" | "<2 x float>" => 8,
            "float" => 4,
            _ => bytes,
        };
        pieces.push((ty, off, psize));
    }
    ArgKindExpand(pieces)
}

/// Registers an argument of this kind needs: (integer, SSE).
pub fn regs_needed(k: &ArgKind) -> (u32, u32) {
    match k {
        ArgKindDirect(t, _) => match t.as_str() {
            "float" | "double" => (0, 1),
            "x86_fp80" => (0, 0),
            _ => (1, 0),
        },
        ArgKindExpand(ps) => {
            let sse = ps
                .iter()
                .filter(|p| matches!(p.0.as_str(), "float" | "double" | "<2 x float>"))
                .count() as u32;
            (ps.len() as u32 - sse, sse)
        }
        _ => (0, 0),
    }
}

/// Classifies every parameter of a call, falling back to memory when the
/// registers run out, as the psABI requires for aggregates.
pub fn classify_params(params: &[type_id], sret: bool) -> Vec<ArgKind> {
    let mut ints: i32 = if sret { 5 } else { 6 };
    let mut sses: i32 = 8;
    let mut out = Vec::new();
    for &p in params {
        let mut k = classify(p);
        let (ni, ns) = regs_needed(&k);
        if matches!(k, ArgKindExpand(_)) && (ni as i32 > ints || ns as i32 > sses) {
            k = ArgKindMemory;
        } else {
            ints -= ni as i32;
            sses -= ns as i32;
        }
        out.push(k);
    }
    out
}

/// The LLVM return type for a classified return value.
pub fn ret_llvm(k: &ArgKind) -> String {
    match k {
        ArgKindIgnore | ArgKindMemory => "void".into(),
        ArgKindDirect(t, _) => t.clone(),
        ArgKindExpand(ps) if ps.len() == 1 => ps[0].0.clone(),
        ArgKindExpand(ps) => {
            format!("{{ {} }}", ps.iter().map(|p| p.0.clone()).collect::<Vec<_>>().join(", "))
        }
    }
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::types::*;
use crate::diag::span;
use crate::lex::sym;
use crate::parse::ast::{BinOp, UnOp};
use crate::parse::literal::Encoding;

pub type local_id = u32;
pub type global_id = u32;
pub type label_id = u32;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Linkage {
    LinkageExternal,
    LinkageInternal,
}
pub use Linkage::*;

#[derive(Clone, Debug)]
pub struct reloc {
    pub offset: u64,
    pub target: RelocTarget,
    pub addend: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelocTarget {
    RelocTargetGlobal(global_id),
    RelocTargetString(u32),
}
pub use RelocTarget::*;

/// A constant byte image with relocations, for static initializers.
#[derive(Clone, Debug, Default)]
pub struct const_data {
    pub bytes: Vec<u8>,
    pub relocs: Vec<reloc>,
}

#[derive(Clone, Debug)]
pub struct global_decl {
    pub name: sym,
    pub ty: qual_type,
    pub is_func: bool,
    pub linkage: Linkage,
    pub symbol: String,
    pub defined: bool,
    pub init: Option<const_data>,
    /// A tentative definition with no initializer: zero-initialized.
    pub tentative: bool,
    pub span: span,
    /// Declared in a C file or C header: unsafe to call from Crust.
    pub c_origin: bool,
    pub safe_attr: bool,
    pub unsafe_fn: bool,
    pub c_abi: bool,
    pub exported: bool,
    /// Module-level `let mut`: reading or writing it needs `unsafe`.
    pub needs_unsafe: bool,
    pub thread_local: bool,
    pub noreturn: bool,
    pub inline_: bool,
    /// The module this declaration was imported from, if any.
    pub imported_from: Option<sym>,
    /// A constant value, for `constexpr` scalars and enumerator-like exports.
    pub const_value: Option<ConstValue>,
    pub is_constexpr: bool,
    pub is_let: bool,
    pub used: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConstValue {
    ConstValueInt(i128),
    ConstValueFloat(f64),
}
pub use ConstValue::*;

#[derive(Clone, Debug)]
pub struct string_lit {
    pub units: Vec<u32>,
    pub encoding: Encoding,
    /// Array type of the literal, including the terminator.
    pub ty: type_id,
}

#[derive(Clone, Debug)]
pub struct local_info {
    pub name: sym,
    pub ty: qual_type,
    pub span: span,
    pub mutable: bool,
    pub is_let: bool,
    pub is_param: bool,
    pub is_temp: bool,
}

#[derive(Clone, Debug)]
pub struct func_body {
    pub global: global_id,
    pub params: Vec<local_id>,
    pub locals: Vec<local_info>,
    pub body: tblock,
    pub span: span,
    /// The body is Crust code (not C mode).
    pub crust: bool,
    pub unsafe_fn: bool,
    pub labels: Vec<label_info>,
    pub ret: qual_type,
    pub name: sym,
}

#[derive(Clone, Debug)]
pub struct label_info {
    pub name: sym,
    pub span: span,
    pub defined: bool,
}

#[derive(Clone, Debug)]
pub struct tblock {
    pub stmts: Vec<tstmt>,
    /// Locals declared directly in this block, in declaration order.
    pub locals: Vec<local_id>,
    pub unsafe_: bool,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct tstmt {
    pub kind: TStmt,
    pub span: span,
}

#[derive(Clone, Debug)]
pub enum TStmt {
    TStmtExpr(texpr),
    /// A local comes into scope, with its initializer.
    TStmtDecl(local_id, Option<tinit>),
    TStmtBlock(tblock),
    TStmtIf(texpr, Box<tstmt>, Option<Box<tstmt>>),
    TStmtWhile(texpr, Box<tstmt>),
    TStmtDoWhile(Box<tstmt>, texpr),
    /// `for`: its own block holds the init declarations.
    TStmtFor(Option<texpr>, Option<texpr>, Box<tstmt>),
    TStmtSwitch(texpr, Box<tstmt>, Vec<(i128, label_id)>, Option<label_id>),
    TStmtLabel(label_id, Box<tstmt>),
    TStmtGoto(label_id),
    TStmtBreak,
    TStmtContinue,
    TStmtReturn(Option<texpr>),
    TStmtNop,
}
pub use TStmt::*;

#[derive(Clone, Debug)]
pub enum PathElem {
    PathElemField(u32, u32),
    PathElemIndex(u64),
}
pub use PathElem::*;

/// A resolved initializer for a local object.
#[derive(Clone, Debug)]
pub enum tinit {
    /// Whole-object initialization from one expression.
    TInitExpr(texpr),
    /// Zero-fill the object (when `zero` is set), then store each element.
    TInitList { zero: bool, items: Vec<(Vec<PathElem>, texpr)> },
}
pub use tinit::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CastKind {
    CastKindNoop,
    CastKindIntToInt,
    CastKindIntToFloat,
    CastKindFloatToInt,
    CastKindFloatToFloat,
    CastKindPtrToPtr,
    CastKindIntToPtr,
    CastKindPtrToInt,
    CastKindToBool,
    CastKindArrayDecay,
    CastKindFuncDecay,
    CastKindNullToPtr,
    CastKindRefToPtr,
    CastKindRefWeaken,
    CastKindArrayRefToSlice,
    CastKindSliceWeaken,
    CastKindToVoid,
}
pub use CastKind::*;

#[derive(Clone, Debug)]
pub struct texpr {
    pub kind: TExpr,
    pub ty: qual_type,
    pub lvalue: bool,
    pub span: span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IncDec {
    IncDecPreInc,
    IncDecPreDec,
    IncDecPostInc,
    IncDecPostDec,
}
pub use IncDec::*;

#[derive(Clone, Debug)]
pub enum TExpr {
    TExprLocal(local_id),
    TExprGlobal(global_id),
    TExprInt(i128),
    TExprFloat(f64),
    TExprString(u32),
    TExprNull,
    /// `*p` for a pointer or borrow. `untracked_ok` is set inside `unsafe`.
    TExprDeref(Box<texpr>),
    TExprMember(Box<texpr>, u32, u32),
    /// Array indexing on an array lvalue: bounds-checked.
    TExprIndex(Box<texpr>, Box<texpr>),
    /// Slice indexing: bounds-checked against the slice length.
    TExprSliceIndex(Box<texpr>, Box<texpr>),
    TExprSliceLen(Box<texpr>),
    TExprAddrOf(Box<texpr>),
    TExprRef(bool, Box<texpr>),
    TExprLoad(Box<texpr>),
    TExprCast(CastKind, Box<texpr>),
    TExprUnary(UnOp, Box<texpr>),
    TExprBinary(BinOp, Box<texpr>, Box<texpr>),
    /// Pointer plus integer, scaled by the pointee size.
    TExprPtrAdd(Box<texpr>, Box<texpr>),
    /// Pointer minus pointer, divided by the pointee size.
    TExprPtrDiff(Box<texpr>, Box<texpr>),
    TExprLogical(bool, Box<texpr>, Box<texpr>),
    TExprAssign(Box<texpr>, Box<texpr>),
    /// `lhs op= rhs`: the operation is done in `op_ty`, then converted back.
    TExprCompoundAssign(BinOp, Box<texpr>, Box<texpr>, qual_type),
    TExprIncDec(IncDec, Box<texpr>),
    TExprCond(Box<texpr>, Box<texpr>, Box<texpr>),
    TExprComma(Box<texpr>, Box<texpr>),
    TExprCall(Box<texpr>, Vec<texpr>, Option<global_id>),
    /// A compound literal: a temporary local, initialized.
    TExprCompound(local_id, Box<tinit>),
    TExprVaStart(Box<texpr>),
    TExprVaEnd(Box<texpr>),
    TExprVaCopy(Box<texpr>, Box<texpr>),
    TExprVaArg(Box<texpr>),
    TExprUnreachable,
    TExprTrap,
}
pub use TExpr::*;

impl texpr {
    pub fn rvalue(kind: TExpr, ty: qual_type, span: span) -> texpr {
        texpr { kind, ty, lvalue: false, span }
    }

    pub fn int_const(v: i128, ty: type_id, span: span) -> texpr {
        texpr::rvalue(TExprInt(v), q(ty), span)
    }

    pub fn error(span: span) -> texpr {
        texpr::rvalue(TExprInt(0), q(error()), span)
    }

    pub fn is_error(&self) -> bool {
        is_error(self.ty.ty)
    }
}

/// Everything sema produces for one translation unit.
#[derive(Default)]
pub struct program {
    pub globals: Vec<global_decl>,
    pub functions: Vec<func_body>,
    pub strings: Vec<string_lit>,
}

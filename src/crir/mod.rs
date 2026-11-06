/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// CrIR: a control-flow graph where moves, borrows, drops, and storage
// lifetimes are explicit. The safety analyses run on it (D-008), and code
// generation lowers it to LLVM IR.
pub mod lower;
pub mod print;

use crate::diag::span;
use crate::lex::sym;
use crate::parse::ast::{BinOp, UnOp};
use crate::sema::tir::{CastKind, global_id};
use crate::sema::types::*;

pub type local = u32;
pub type bb_id = u32;

/// The return slot is always local 0.
pub const ReturnSlot: local = 0;

#[derive(Clone, Debug)]
pub struct local_decl {
    pub ty: qual_type,
    pub name: Option<sym>,
    pub span: span,
    pub mutable: bool,
    pub is_let: bool,
    /// A variable the user declared (not a temporary).
    pub user: bool,
    pub is_param: bool,
    /// Zero-filled at function entry, so reads never see indeterminate bytes.
    pub zero_init: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlaceBase {
    PlaceBaseLocal(local),
    PlaceBaseGlobal(global_id),
    PlaceBaseString(u32),
}
pub use PlaceBase::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Proj {
    ProjDeref,
    ProjField(u32, u32),
    /// Index by a local holding an `i64`. On an array place this selects an
    /// element; on a slice place it selects an element of the slice.
    ProjIndex(local),
    ProjConstIndex(u64),
}
pub use Proj::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct place {
    pub base: PlaceBase,
    pub proj: Vec<Proj>,
}

impl place {
    pub fn local(l: local) -> place {
        place { base: PlaceBaseLocal(l), proj: Vec::new() }
    }

    pub fn project(&self, p: Proj) -> place {
        let mut n = self.clone();
        n.proj.push(p);
        n
    }

    pub fn as_local(&self) -> Option<local> {
        match self.base {
            PlaceBaseLocal(l) if self.proj.is_empty() => Some(l),
            _ => None,
        }
    }

    pub fn root_local(&self) -> Option<local> {
        match self.base {
            PlaceBaseLocal(l) => Some(l),
            _ => None,
        }
    }

    pub fn has_deref(&self) -> bool {
        self.proj.iter().any(|p| matches!(p, ProjDeref))
    }
}

#[derive(Clone, Debug)]
pub enum Constant {
    ConstInt(i128, type_id),
    ConstFloat(f64, type_id),
    ConstNull(type_id),
    ConstGlobalAddr(global_id),
    ConstStringAddr(u32),
}
pub use Constant::*;

#[derive(Clone, Debug)]
pub enum Operand {
    OperandCopy(place),
    OperandMove(place),
    OperandConst(Constant),
}
pub use Operand::*;

impl Operand {
    pub fn place(&self) -> Option<&place> {
        match self {
            OperandCopy(p) | OperandMove(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Rvalue {
    RvalueUse(Operand),
    RvalueBinary(BinOp, Operand, Operand),
    RvalueUnary(UnOp, Operand),
    RvalueCast(CastKind, Operand, qual_type),
    /// Pointer plus an `i64` element count.
    RvaluePtrAdd(Operand, Operand),
    RvaluePtrDiff(Operand, Operand),
    /// `&place`: a raw pointer.
    RvalueAddrOf(place),
    /// `ref place` or `ref mut place`.
    RvalueRef(bool, place),
    /// A slice of `n` elements from a borrow of an array.
    RvalueArrayToSlice(Operand, u64),
    RvalueSliceLen(place),
    RvalueVaArg(Operand),
    /// All-zero bytes of the destination's type.
    RvalueZero,
}
pub use Rvalue::*;

#[derive(Clone, Debug)]
pub enum AssertKind {
    AssertKindBounds,
    AssertKindDivZero,
}
pub use AssertKind::*;

#[derive(Clone, Debug)]
pub enum StatementKind {
    StatementAssign(place, Rvalue),
    StatementStorageLive(local),
    StatementStorageDead(local),
    /// Drop the place if it is initialized. Lowering emits these at scope
    /// ends and before assignments; drop elaboration replaces them.
    StatementDropIfInit(place),
    StatementDrop(place),
    /// Drop the place if the flag local is set, then clear the flag.
    StatementDropIfFlag(place, local),
    StatementSetFlag(local, bool),
    StatementCall {
        dest: Option<place>,
        func: Operand,
        args: Vec<Operand>,
        direct: Option<global_id>,
    },
    /// Trap unless `cond` (an `i1`-like operand) is true.
    StatementAssert(Operand, AssertKind),
    StatementVaStart(Operand),
    StatementVaEnd(Operand),
    StatementVaCopy(Operand, Operand),
    StatementNop,
}
pub use StatementKind::*;

#[derive(Clone, Debug)]
pub struct statement {
    pub kind: StatementKind,
    pub span: span,
    /// Inside `unsafe`, or in a C-mode function.
    pub unchecked: bool,
}

#[derive(Clone, Debug)]
pub enum Terminator {
    TermGoto(bb_id),
    TermSwitch(Operand, Vec<(i128, bb_id)>, bb_id),
    TermReturn,
    TermUnreachable,
    TermTrap,
}
pub use Terminator::*;

impl Terminator {
    pub fn successors(&self) -> Vec<bb_id> {
        match self {
            TermGoto(b) => vec![*b],
            TermSwitch(_, cases, d) => {
                let mut v: Vec<bb_id> = cases.iter().map(|c| c.1).collect();
                v.push(*d);
                v
            }
            _ => vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct basic_block {
    pub stmts: Vec<statement>,
    pub term: Terminator,
    pub term_span: span,
}

#[derive(Clone, Debug)]
pub struct body {
    pub func: global_id,
    pub name: sym,
    pub locals: Vec<local_decl>,
    /// Locals 1..=arg_count are the parameters.
    pub arg_count: usize,
    pub blocks: Vec<basic_block>,
    pub crust: bool,
    pub span: span,
    pub ret: qual_type,
}

impl body {
    pub fn place_ty(&self, p: &place, prog: &crate::sema::tir::program) -> qual_type {
        let mut t = match p.base {
            PlaceBaseLocal(l) => self.locals[l as usize].ty,
            PlaceBaseGlobal(g) => prog.globals[g as usize].ty,
            PlaceBaseString(s) => q(prog.strings[s as usize].ty),
        };
        for pr in &p.proj {
            t = proj_ty(t, pr);
        }
        t
    }

    pub fn predecessors(&self) -> Vec<Vec<bb_id>> {
        let mut preds = vec![Vec::new(); self.blocks.len()];
        for (i, b) in self.blocks.iter().enumerate() {
            for s in b.term.successors() {
                preds[s as usize].push(i as bb_id);
            }
        }
        preds
    }
}

pub fn proj_ty(t: qual_type, pr: &Proj) -> qual_type {
    match (pr, t.kind()) {
        (ProjDeref, TypeKindPointer(p)) => p,
        (ProjDeref, TypeKindRef { pointee, .. }) => pointee,
        (ProjField(r, i), _) => with_record(*r, |rd| rd.fields[*i as usize].ty),
        (ProjIndex(_) | ProjConstIndex(_), TypeKindArray(e, _)) => e,
        (ProjIndex(_) | ProjConstIndex(_), TypeKindSlice { elem, .. }) => elem,
        _ => q(error()),
    }
}

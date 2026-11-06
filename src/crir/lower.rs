/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::*;
use crate::lex::SymMain;
use crate::sema::consteval;
use crate::sema::tir::*;

#[derive(Clone, Copy)]
enum ScopeKind {
    ScopeKindBlock,
    ScopeKindLoop { brk: bb_id, cont: bb_id },
    ScopeKindSwitch { brk: bb_id },
}
use ScopeKind::*;

struct scope_frame {
    id: u32,
    locals: Vec<local>,
    kind: ScopeKind,
}

struct pending_goto {
    from: bb_id,
    stack: Vec<(u32, usize)>,
    label: label_id,
    span: span,
}

struct builder<'a> {
    prog: &'a program,
    b: body,
    cur: bb_id,
    scopes: Vec<scope_frame>,
    next_scope: u32,
    label_bbs: Vec<bb_id>,
    label_stacks: Vec<Option<Vec<u32>>>,
    gotos: Vec<pending_goto>,
    unsafe_depth: u32,
    unsafe_fn: bool,
    owned_temps: Vec<local>,
    /// Locals of every scope ever opened, by scope id, for goto exits.
    closed_frames: Vec<(u32, Vec<local>)>,
}

/// Lowers one function from the typed IR to CrIR.
pub fn lower_function(f: &func_body, prog: &program) -> body {
    let mut locals = vec![local_decl {
        ty: f.ret,
        name: None,
        span: f.span,
        mutable: true,
        is_let: false,
        user: false,
        is_param: false,
        zero_init: false,
    }];
    for l in &f.locals {
        locals.push(local_decl {
            ty: l.ty,
            name: if l.name == crate::lex::SymEmpty { None } else { Some(l.name) },
            span: l.span,
            mutable: l.mutable,
            is_let: l.is_let,
            user: !l.is_temp,
            is_param: l.is_param,
            zero_init: false,
        });
    }
    let b = body {
        func: f.global,
        name: f.name,
        locals,
        arg_count: f.params.len(),
        blocks: Vec::new(),
        crust: f.crust,
        span: f.span,
        ret: f.ret,
    };
    let mut bl = builder {
        prog,
        b,
        cur: 0,
        scopes: Vec::new(),
        next_scope: 0,
        label_bbs: Vec::new(),
        label_stacks: vec![None; f.labels.len()],
        gotos: Vec::new(),
        unsafe_depth: 0,
        unsafe_fn: f.unsafe_fn,
        owned_temps: Vec::new(),
        closed_frames: Vec::new(),
    };
    bl.cur = bl.new_block();
    for _ in &f.labels {
        let b = bl.new_block();
        bl.label_bbs.push(b);
    }
    let is_main = f.name == SymMain;
    if !is_void(f.ret.ty) && (!f.crust || is_main) {
        // C mode and `main`: falling off the end returns zero, never garbage.
        bl.emit(StatementAssign(place::local(ReturnSlot), RvalueZero), f.span);
    }
    let params: Vec<local> = f.params.iter().map(|p| p + 1).collect();
    bl.push_scope(ScopeKindBlock);
    bl.scopes.last_mut().unwrap().locals.extend(params);
    bl.block(&f.body);
    let end = span { file: f.span.file, lo: f.span.hi.saturating_sub(1), hi: f.span.hi };
    bl.exit_scopes_to(0, end);
    bl.scopes.pop();
    bl.terminate(TermReturn, end);
    bl.resolve_gotos();
    bl.b
}

impl builder<'_> {
    fn new_block(&mut self) -> bb_id {
        self.b.blocks.push(basic_block {
            stmts: Vec::new(),
            term: TermUnreachable,
            term_span: self.b.span,
        });
        self.b.blocks.len() as bb_id - 1
    }

    fn unchecked(&self) -> bool {
        !self.b.crust || self.unsafe_depth > 0 || self.unsafe_fn
    }

    fn emit(&mut self, kind: StatementKind, sp: span) {
        let unchecked = self.unchecked();
        self.b.blocks[self.cur as usize].stmts.push(statement { kind, span: sp, unchecked });
    }

    /// Ends the current block. Code after it goes into a fresh block with no
    /// predecessors.
    fn terminate(&mut self, t: Terminator, sp: span) {
        self.b.blocks[self.cur as usize].term = t;
        self.b.blocks[self.cur as usize].term_span = sp;
        self.cur = self.new_block();
    }

    fn goto(&mut self, target: bb_id, sp: span) {
        self.terminate(TermGoto(target), sp);
        self.cur = target;
    }

    fn temp(&mut self, ty: qual_type, sp: span) -> local {
        self.b.locals.push(local_decl {
            ty: ty.unqual(),
            name: None,
            span: sp,
            mutable: true,
            is_let: false,
            user: false,
            is_param: false,
            zero_init: false,
        });
        let l = self.b.locals.len() as local - 1;
        if is_owned(ty.ty) {
            self.owned_temps.push(l);
        }
        l
    }

    fn assign(&mut self, p: place, rv: Rvalue, sp: span) {
        self.emit(StatementAssign(p, rv), sp);
    }

    /// Drops owned temporaries at the end of a full expression.
    fn flush_temps(&mut self, sp: span) {
        for t in std::mem::take(&mut self.owned_temps).into_iter().rev() {
            self.emit(StatementDropIfInit(place::local(t)), sp);
        }
    }

    // ---- scopes ----

    fn push_scope(&mut self, kind: ScopeKind) {
        self.next_scope += 1;
        self.scopes.push(scope_frame { id: self.next_scope, locals: Vec::new(), kind });
    }

    fn record_local(&mut self, id: u32, l: local) {
        match self.closed_frames.iter_mut().find(|(f, _)| *f == id) {
            Some((_, v)) => v.push(l),
            None => self.closed_frames.push((id, vec![l])),
        }
    }

    fn exit_frame_locals(&mut self, locals: &[local], sp: span) {
        for &l in locals.iter().rev() {
            if is_owned(self.b.locals[l as usize].ty.ty) {
                self.emit(StatementDropIfInit(place::local(l)), sp);
            }
            self.emit(StatementStorageDead(l), sp);
        }
    }

    /// Emits drops and storage ends for every scope above depth `keep`.
    fn exit_scopes_to(&mut self, keep: usize, sp: span) {
        let frames: Vec<Vec<local>> =
            self.scopes[keep..].iter().rev().map(|f| f.locals.clone()).collect();
        for locals in frames {
            self.exit_frame_locals(&locals, sp);
        }
    }

    fn block(&mut self, b: &tblock) {
        if b.unsafe_ {
            self.unsafe_depth += 1;
        }
        for s in &b.stmts {
            self.stmt(s);
        }
        if b.unsafe_ {
            self.unsafe_depth -= 1;
        }
    }

    fn scoped_block(&mut self, b: &tblock) {
        self.push_scope(ScopeKindBlock);
        self.block(b);
        let end = span { file: b.span.file, lo: b.span.hi.saturating_sub(1), hi: b.span.hi };
        let frame = self.scopes.pop().unwrap();
        if b.unsafe_ {
            self.unsafe_depth += 1;
        }
        self.exit_frame_locals(&frame.locals, end);
        if b.unsafe_ {
            self.unsafe_depth -= 1;
        }
    }

    // ---- statements ----

    fn stmt(&mut self, s: &tstmt) {
        let sp = s.span;
        match &s.kind {
            TStmtNop => {}
            TStmtExpr(e) => {
                self.effect(e);
                self.flush_temps(sp);
            }
            TStmtDecl(l, init) => {
                let cl = l + 1;
                self.emit(StatementStorageLive(cl), sp);
                if let Some(fr) = self.scopes.last_mut() {
                    fr.locals.push(cl);
                    let id = fr.id;
                    self.record_local(id, cl);
                }
                if let Some(i) = init {
                    self.init_place(place::local(cl), i, sp);
                }
                self.flush_temps(sp);
            }
            TStmtBlock(b) => self.scoped_block(b),
            TStmtIf(c, t, e) => {
                let (tb, fb, join) = (self.new_block(), self.new_block(), self.new_block());
                self.cond(c, tb, fb);
                self.cur = tb;
                self.stmt(t);
                self.goto(join, sp);
                self.cur = fb;
                if let Some(e) = e {
                    self.stmt(e);
                }
                self.goto(join, sp);
            }
            TStmtWhile(c, body) => {
                let (head, bodyb, exit) = (self.new_block(), self.new_block(), self.new_block());
                self.goto(head, sp);
                self.cond(c, bodyb, exit);
                self.cur = bodyb;
                self.loop_body(body, exit, head);
                self.goto(head, sp);
                self.cur = exit;
            }
            TStmtDoWhile(body, c) => {
                let (bodyb, contb, exit) = (self.new_block(), self.new_block(), self.new_block());
                self.goto(bodyb, sp);
                self.loop_body(body, exit, contb);
                self.goto(contb, sp);
                self.cond(c, bodyb, exit);
                self.cur = exit;
            }
            TStmtFor(c, step, body) => {
                let (head, bodyb, stepb, exit) =
                    (self.new_block(), self.new_block(), self.new_block(), self.new_block());
                self.goto(head, sp);
                match c {
                    Some(c) => self.cond(c, bodyb, exit),
                    None => self.goto(bodyb, sp),
                }
                self.cur = bodyb;
                self.loop_body(body, exit, stepb);
                self.goto(stepb, sp);
                if let Some(st) = step {
                    self.effect(st);
                    self.flush_temps(sp);
                }
                self.goto(head, sp);
                self.cur = exit;
            }
            TStmtSwitch(v, body, cases, default) => {
                let op = self.operand(v);
                self.flush_temps(sp);
                let exit = self.new_block();
                let targets: Vec<(i128, bb_id)> =
                    cases.iter().map(|(val, l)| (*val, self.label_bbs[*l as usize])).collect();
                let d = default.map(|l| self.label_bbs[l as usize]).unwrap_or(exit);
                self.terminate(TermSwitch(op, targets, d), sp);
                self.push_scope(ScopeKindSwitch { brk: exit });
                self.stmt(body);
                let frame = self.scopes.pop().unwrap();
                self.exit_frame_locals(&frame.locals, sp);
                self.goto(exit, sp);
            }
            TStmtLabel(l, inner) => {
                let target = self.label_bbs[*l as usize];
                self.goto(target, sp);
                self.label_stacks[*l as usize] = Some(self.scopes.iter().map(|f| f.id).collect());
                self.stmt(inner);
            }
            TStmtGoto(l) => {
                let stack = self.scopes.iter().map(|f| (f.id, f.locals.len())).collect();
                self.gotos.push(pending_goto { from: self.cur, stack, label: *l, span: sp });
                self.cur = self.new_block();
            }
            TStmtBreak | TStmtContinue => {
                let is_break = matches!(s.kind, TStmtBreak);
                let mut target = None;
                for (i, f) in self.scopes.iter().enumerate().rev() {
                    match (f.kind, is_break) {
                        (ScopeKindLoop { brk, .. }, true) | (ScopeKindSwitch { brk }, true) => {
                            target = Some((i, brk));
                            break;
                        }
                        (ScopeKindLoop { cont, .. }, false) => {
                            target = Some((i, cont));
                            break;
                        }
                        _ => {}
                    }
                }
                let Some((depth, bb)) = target else { return };
                self.exit_scopes_to(depth + 1, sp);
                self.terminate(TermGoto(bb), sp);
            }
            TStmtReturn(e) => {
                if let Some(e) = e {
                    self.expr_into(e, place::local(ReturnSlot));
                }
                self.flush_temps(sp);
                self.exit_scopes_to(0, sp);
                self.terminate(TermReturn, sp);
            }
        }
    }

    fn loop_body(&mut self, body: &tstmt, brk: bb_id, cont: bb_id) {
        self.push_scope(ScopeKindLoop { brk, cont });
        self.stmt(body);
        let frame = self.scopes.pop().unwrap();
        self.exit_frame_locals(&frame.locals, body.span);
    }

    fn resolve_gotos(&mut self) {
        for g in std::mem::take(&mut self.gotos) {
            let Some(target_stack) = self.label_stacks[g.label as usize].clone() else {
                continue;
            };
            let common = g.stack.iter().zip(&target_stack).take_while(|(a, b)| a.0 == **b).count();
            self.cur = g.from;
            // Exit the scopes the goto leaves, dropping the locals declared
            // so far in each.
            let frames: Vec<Vec<local>> =
                g.stack[common..].iter().rev().map(|(id, n)| self.frame_locals(*id, *n)).collect();
            for locals in frames {
                self.exit_frame_locals(&locals, g.span);
            }
            let target = self.label_bbs[g.label as usize];
            self.b.blocks[self.cur as usize].term = TermGoto(target);
            self.b.blocks[self.cur as usize].term_span = g.span;
        }
    }

    /// Locals of a (possibly closed) scope, recovered from StorageLive order.
    fn frame_locals(&self, id: u32, n: usize) -> Vec<local> {
        self.closed_frames
            .iter()
            .find(|(fid, _)| *fid == id)
            .map(|(_, l)| l[..n.min(l.len())].to_vec())
            .unwrap_or_default()
    }

    // ---- conditions ----

    fn cond(&mut self, e: &texpr, t: bb_id, f: bb_id) {
        match &e.kind {
            TExprLogical(and, a, b) => {
                let mid = self.new_block();
                if *and {
                    self.cond(a, mid, f);
                } else {
                    self.cond(a, t, mid);
                }
                self.cur = mid;
                self.cond(b, t, f);
            }
            TExprUnary(crate::parse::ast::UnOpNot, x) => self.cond(x, f, t),
            _ => {
                let op = self.truth(e);
                self.flush_temps(e.span);
                self.terminate(TermSwitch(op, vec![(0, f)], t), e.span);
            }
        }
    }

    /// An integer operand that is zero exactly when `e` is false.
    fn truth(&mut self, e: &texpr) -> Operand {
        let op = self.operand(e);
        if is_integer(e.ty.ty) {
            return op;
        }
        let zero = if is_float(e.ty.ty) {
            OperandConst(ConstFloat(0.0, e.ty.ty))
        } else {
            OperandConst(ConstNull(e.ty.ty))
        };
        let t = self.temp(q(int()), e.span);
        self.assign(place::local(t), RvalueBinary(crate::parse::ast::BinOpNe, op, zero), e.span);
        OperandCopy(place::local(t))
    }

    // ---- expressions ----

    fn effect(&mut self, e: &texpr) {
        match &e.kind {
            TExprLoad(_) if is_owned(e.ty.ty) => {}
            TExprComma(a, b) => {
                self.effect(a);
                self.effect(b);
            }
            TExprCast(CastKindToVoid, x) => self.effect(x),
            TExprCall(..)
            | TExprCond(..)
            | TExprLogical(..)
            | TExprAssign(..)
            | TExprCompoundAssign(..)
            | TExprIncDec(..)
            | TExprVaStart(_)
            | TExprVaEnd(_)
            | TExprVaCopy(..)
            | TExprUnreachable
            | TExprTrap => {
                if is_void(e.ty.ty)
                    || matches!(e.kind, TExprAssign(..) | TExprCompoundAssign(..) | TExprIncDec(..))
                {
                    self.void_expr(e);
                } else {
                    let t = self.temp(e.ty, e.span);
                    self.expr_into(e, place::local(t));
                }
            }
            _ => {
                if is_void(e.ty.ty) {
                    self.void_expr(e);
                } else if e.lvalue {
                    let _ = self.place(e);
                } else {
                    let _ = self.operand(e);
                }
            }
        }
    }

    /// Lowers an expression whose value is not needed.
    fn void_expr(&mut self, e: &texpr) {
        let sp = e.span;
        match &e.kind {
            TExprCall(f, args, direct) => self.call(f, args, *direct, None, sp),
            TExprAssign(l, r) => {
                self.assignment(l, r, sp);
            }
            TExprCompoundAssign(op, l, r, op_ty) => {
                self.compound_assign(*op, l, r, *op_ty, sp);
            }
            TExprIncDec(k, l) => {
                self.inc_dec(*k, l, sp);
            }
            TExprCond(c, a, b) => {
                let (tb, fb, join) = (self.new_block(), self.new_block(), self.new_block());
                self.cond(c, tb, fb);
                self.cur = tb;
                self.effect(a);
                self.goto(join, sp);
                self.cur = fb;
                self.effect(b);
                self.goto(join, sp);
            }
            TExprComma(a, b) => {
                self.effect(a);
                self.effect(b);
            }
            TExprVaStart(ap) => {
                let op = self.operand(ap);
                self.emit(StatementVaStart(op), sp);
            }
            TExprVaEnd(ap) => {
                let op = self.operand(ap);
                self.emit(StatementVaEnd(op), sp);
            }
            TExprVaCopy(d, s) => {
                let d = self.operand(d);
                let s = self.operand(s);
                self.emit(StatementVaCopy(d, s), sp);
            }
            TExprUnreachable => self.terminate(TermUnreachable, sp),
            TExprTrap => self.terminate(TermTrap, sp),
            TExprCast(_, x) => self.effect(x),
            _ => self.effect(e),
        }
    }

    pub fn operand(&mut self, e: &texpr) -> Operand {
        match &e.kind {
            TExprInt(v) => OperandConst(ConstInt(*v, e.ty.ty)),
            TExprFloat(f) => OperandConst(ConstFloat(*f, e.ty.ty)),
            TExprNull => OperandConst(ConstNull(e.ty.ty)),
            TExprLoad(lv) => {
                let p = self.place(lv);
                if is_owned(lv.ty.ty) { OperandMove(p) } else { OperandCopy(p) }
            }
            TExprCast(CastKindFuncDecay, inner) if matches!(inner.kind, TExprGlobal(_)) => {
                let TExprGlobal(g) = inner.kind else { unreachable!() };
                OperandConst(ConstGlobalAddr(g))
            }
            _ => {
                let t = self.temp(e.ty, e.span);
                self.expr_into(e, place::local(t));
                if is_owned(e.ty.ty) {
                    OperandMove(place::local(t))
                } else {
                    OperandCopy(place::local(t))
                }
            }
        }
    }

    /// An operand converted to `i64`, for indexing and pointer arithmetic.
    fn index_operand(&mut self, e: &texpr) -> local {
        let op = self.operand(e);
        let t = self.temp(q(int_of(IntKindLong)), e.span);
        let rv = if int_kind(e.ty.ty).is_some_and(|k| int_size(k) == 8) {
            RvalueUse(op)
        } else {
            RvalueCast(CastKindIntToInt, op, q(int_of(IntKindLong)))
        };
        self.assign(place::local(t), rv, e.span);
        t
    }

    fn bounds_check(&mut self, idx: local, len: Operand, sp: span) {
        let ul = q(int_of(IntKindULong));
        let ui = self.temp(ul, sp);
        self.assign(
            place::local(ui),
            RvalueCast(CastKindIntToInt, OperandCopy(place::local(idx)), ul),
            sp,
        );
        let c = self.temp(q(int()), sp);
        self.assign(
            place::local(c),
            RvalueBinary(crate::parse::ast::BinOpLt, OperandCopy(place::local(ui)), len),
            sp,
        );
        self.emit(StatementAssert(OperandCopy(place::local(c)), AssertKindBounds), sp);
    }

    pub fn place(&mut self, e: &texpr) -> place {
        let sp = e.span;
        match &e.kind {
            TExprLocal(l) => place::local(l + 1),
            TExprGlobal(g) => place { base: PlaceBaseGlobal(*g), proj: vec![] },
            TExprString(s) => place { base: PlaceBaseString(*s), proj: vec![] },
            TExprDeref(p) => {
                let op = self.operand(p);
                let base = match op {
                    OperandCopy(pl) | OperandMove(pl) => pl,
                    OperandConst(_) => {
                        let t = self.temp(p.ty, sp);
                        self.assign(place::local(t), RvalueUse(op), sp);
                        place::local(t)
                    }
                };
                base.project(ProjDeref)
            }
            TExprMember(b, r, i) => {
                let bp = if b.lvalue { self.place(b) } else { self.materialize(b) };
                bp.project(ProjField(*r, *i))
            }
            TExprIndex(b, i) => {
                let bp = self.place(b);
                let len = match b.ty.kind() {
                    TypeKindArray(_, n) => n,
                    _ => None,
                };
                if let (Some(ConstValueInt(v)), Some(n)) = (consteval::eval(i), len) {
                    if v >= 0 && (v as u64) < n {
                        return bp.project(ProjConstIndex(v as u64));
                    }
                }
                let idx = self.index_operand(i);
                if let Some(n) = len {
                    self.bounds_check(idx, OperandConst(ConstInt(n as i128, size_t())), sp);
                }
                bp.project(ProjIndex(idx))
            }
            TExprSliceIndex(s, i) => {
                let sp_place = match self.operand(s) {
                    OperandCopy(p) | OperandMove(p) => p,
                    op => {
                        let t = self.temp(s.ty, sp);
                        self.assign(place::local(t), RvalueUse(op), sp);
                        place::local(t)
                    }
                };
                let idx = self.index_operand(i);
                let len = self.temp(q(size_t()), sp);
                self.assign(place::local(len), RvalueSliceLen(sp_place.clone()), sp);
                self.bounds_check(idx, OperandCopy(place::local(len)), sp);
                sp_place.project(ProjIndex(idx))
            }
            TExprCompound(l, init) => {
                let p = place::local(l + 1);
                self.emit(StatementStorageLive(l + 1), sp);
                if let Some(fr) = self.scopes.last_mut() {
                    if !fr.locals.contains(&(l + 1)) {
                        fr.locals.push(l + 1);
                        let id = fr.id;
                        self.record_local(id, l + 1);
                    }
                }
                self.init_place(p.clone(), init, sp);
                p
            }
            _ => self.materialize(e),
        }
    }

    fn materialize(&mut self, e: &texpr) -> place {
        let t = self.temp(e.ty, e.span);
        self.expr_into(e, place::local(t));
        place::local(t)
    }

    fn init_place(&mut self, p: place, init: &tinit, sp: span) {
        match init {
            TInitExpr(e) => self.expr_into(e, p),
            TInitList { zero, items } => {
                if *zero {
                    self.assign(p.clone(), RvalueZero, sp);
                }
                for (path, e) in items {
                    let mut dst = p.clone();
                    for pe in path {
                        dst = dst.project(match pe {
                            PathElemField(r, i) => ProjField(*r, *i),
                            PathElemIndex(i) => ProjConstIndex(*i),
                        });
                    }
                    self.expr_into(e, dst);
                }
            }
        }
    }

    /// Evaluates `e` and stores its value into `dest`.
    fn expr_into(&mut self, e: &texpr, dest: place) {
        let sp = e.span;
        match &e.kind {
            TExprCall(f, args, direct) => self.call(f, args, *direct, Some(dest), sp),
            TExprCond(c, a, b) => {
                let (tb, fb, join) = (self.new_block(), self.new_block(), self.new_block());
                self.cond(c, tb, fb);
                self.cur = tb;
                self.expr_into(a, dest.clone());
                self.goto(join, sp);
                self.cur = fb;
                self.expr_into(b, dest);
                self.goto(join, sp);
            }
            TExprLogical(..) => {
                let (tb, fb, join) = (self.new_block(), self.new_block(), self.new_block());
                self.cond(e, tb, fb);
                self.cur = tb;
                self.assign(dest.clone(), RvalueUse(OperandConst(ConstInt(1, int()))), sp);
                self.goto(join, sp);
                self.cur = fb;
                self.assign(dest, RvalueUse(OperandConst(ConstInt(0, int()))), sp);
                self.goto(join, sp);
            }
            TExprComma(a, b) => {
                self.effect(a);
                self.expr_into(b, dest);
            }
            TExprAssign(l, r) => {
                let p = self.assignment(l, r, sp);
                self.assign(dest, RvalueUse(OperandCopy(p)), sp);
            }
            TExprCompoundAssign(op, l, r, op_ty) => {
                let p = self.compound_assign(*op, l, r, *op_ty, sp);
                self.assign(dest, RvalueUse(OperandCopy(p)), sp);
            }
            TExprIncDec(k, l) => {
                let v = self.inc_dec(*k, l, sp);
                self.assign(dest, RvalueUse(v), sp);
            }
            TExprCompound(..) | TExprMember(..) | TExprIndex(..) | TExprSliceIndex(..)
            | TExprDeref(_) | TExprLocal(_) | TExprGlobal(_) | TExprString(_) => {
                // An lvalue used as a value (aggregate copy).
                let p = self.place(e);
                let op = if is_owned(e.ty.ty) { OperandMove(p) } else { OperandCopy(p) };
                self.assign(dest, RvalueUse(op), sp);
            }
            TExprVaStart(_) | TExprVaEnd(_) | TExprVaCopy(..) | TExprUnreachable | TExprTrap => {
                self.void_expr(e)
            }
            _ => {
                let rv = self.rvalue(e);
                self.assign(dest, rv, sp);
            }
        }
    }

    fn rvalue(&mut self, e: &texpr) -> Rvalue {
        let sp = e.span;
        match &e.kind {
            TExprInt(_) | TExprFloat(_) | TExprNull | TExprLoad(_) => RvalueUse(self.operand(e)),
            TExprCast(CastKindArrayDecay, lv) => RvalueAddrOf(self.place(lv)),
            TExprCast(CastKindFuncDecay, lv) => match &lv.kind {
                TExprGlobal(g) => RvalueUse(OperandConst(ConstGlobalAddr(*g))),
                TExprDeref(p) => RvalueUse(self.operand(p)),
                _ => RvalueAddrOf(self.place(lv)),
            },
            TExprCast(CastKindArrayRefToSlice, x) => {
                let n = match pointee(x.ty.ty).map(|p| p.kind()) {
                    Some(TypeKindArray(_, Some(n))) => n,
                    _ => 0,
                };
                RvalueArrayToSlice(self.operand(x), n)
            }
            TExprCast(k, x) => RvalueCast(*k, self.operand(x), e.ty),
            TExprUnary(op, x) => RvalueUnary(*op, self.operand(x)),
            TExprBinary(op, a, b) => {
                let x = self.operand(a);
                let y = self.operand(b);
                if matches!(op, crate::parse::ast::BinOpDiv | crate::parse::ast::BinOpRem)
                    && is_integer(b.ty.ty)
                {
                    let nz = !matches!(consteval::eval(b), Some(ConstValueInt(v)) if v != 0);
                    if nz {
                        let c = self.temp(q(int()), sp);
                        self.assign(
                            place::local(c),
                            RvalueBinary(
                                crate::parse::ast::BinOpNe,
                                y.clone(),
                                OperandConst(ConstInt(0, b.ty.ty)),
                            ),
                            sp,
                        );
                        self.emit(
                            StatementAssert(OperandCopy(place::local(c)), AssertKindDivZero),
                            sp,
                        );
                    }
                }
                RvalueBinary(*op, x, y)
            }
            TExprPtrAdd(p, i) => {
                let po = self.operand(p);
                let idx = self.index_operand(i);
                RvaluePtrAdd(po, OperandCopy(place::local(idx)))
            }
            TExprPtrDiff(a, b) => RvaluePtrDiff(self.operand(a), self.operand(b)),
            TExprAddrOf(lv) => RvalueAddrOf(self.place(lv)),
            TExprRef(m, lv) => RvalueRef(*m, self.place(lv)),
            TExprSliceLen(s) => {
                let p = match self.operand(s) {
                    OperandCopy(p) | OperandMove(p) => p,
                    op => {
                        let t = self.temp(s.ty, sp);
                        self.assign(place::local(t), RvalueUse(op), sp);
                        place::local(t)
                    }
                };
                RvalueSliceLen(p)
            }
            TExprVaArg(ap) => RvalueVaArg(self.operand(ap)),
            _ => {
                let t = self.temp(e.ty, sp);
                self.expr_into(e, place::local(t));
                RvalueUse(if is_owned(e.ty.ty) {
                    OperandMove(place::local(t))
                } else {
                    OperandCopy(place::local(t))
                })
            }
        }
    }

    fn call(
        &mut self,
        f: &texpr,
        args: &[texpr],
        direct: Option<global_id>,
        dest: Option<place>,
        sp: span,
    ) {
        let func = match direct {
            Some(g) => OperandConst(ConstGlobalAddr(g)),
            None => self.operand(f),
        };
        let ops: Vec<Operand> = args.iter().map(|a| self.operand(a)).collect();
        let dest = match dest {
            Some(d) => Some(d),
            None => {
                let rt = func_of(f.ty.ty).map(|ft| ft.ret).unwrap_or(q(void()));
                if is_void(rt.ty) { None } else { Some(place::local(self.temp(rt, sp))) }
            }
        };
        self.emit(StatementCall { dest, func, args: ops, direct }, sp);
        let noreturn = direct.is_some_and(|g| self.prog.globals[g as usize].noreturn);
        if noreturn {
            self.terminate(TermUnreachable, sp);
        }
    }

    /// Lowers `l = r` and returns the assigned place.
    fn assignment(&mut self, l: &texpr, r: &texpr, sp: span) -> place {
        let rv = self.rvalue_for_assign(r);
        let p = self.place(l);
        if is_owned(l.ty.ty) {
            if p.has_deref() {
                self.emit(StatementDrop(p.clone()), sp);
            } else {
                self.emit(StatementDropIfInit(p.clone()), sp);
            }
        }
        self.assign(p.clone(), rv, sp);
        p
    }

    fn rvalue_for_assign(&mut self, r: &texpr) -> Rvalue {
        match &r.kind {
            TExprCall(..)
            | TExprCond(..)
            | TExprLogical(..)
            | TExprComma(..)
            | TExprAssign(..)
            | TExprCompoundAssign(..)
            | TExprIncDec(..) => {
                let t = self.temp(r.ty, r.span);
                self.expr_into(r, place::local(t));
                RvalueUse(if is_owned(r.ty.ty) {
                    OperandMove(place::local(t))
                } else {
                    OperandCopy(place::local(t))
                })
            }
            _ => self.rvalue(r),
        }
    }

    fn compound_assign(
        &mut self,
        op: crate::parse::ast::BinOp,
        l: &texpr,
        r: &texpr,
        op_ty: qual_type,
        sp: span,
    ) -> place {
        let rhs = self.operand(r);
        let p = self.place(l);
        let lt = l.ty.unqual();
        if is_pointer(lt.ty) {
            let idx = self.temp(q(int_of(IntKindLong)), sp);
            let rv = if op == crate::parse::ast::BinOpSub {
                RvalueUnary(crate::parse::ast::UnOpNeg, rhs)
            } else {
                RvalueUse(rhs)
            };
            self.assign(place::local(idx), rv, sp);
            self.assign(
                p.clone(),
                RvaluePtrAdd(OperandCopy(p.clone()), OperandCopy(place::local(idx))),
                sp,
            );
            return p;
        }
        let cur = self.temp(op_ty, sp);
        let load = if lt.ty == op_ty.ty {
            RvalueUse(OperandCopy(p.clone()))
        } else {
            RvalueCast(cast_kind(lt.ty, op_ty.ty), OperandCopy(p.clone()), op_ty)
        };
        self.assign(place::local(cur), load, sp);
        if matches!(op, crate::parse::ast::BinOpDiv | crate::parse::ast::BinOpRem)
            && is_integer(op_ty.ty)
        {
            let c = self.temp(q(int()), sp);
            self.assign(
                place::local(c),
                RvalueBinary(
                    crate::parse::ast::BinOpNe,
                    rhs.clone(),
                    OperandConst(ConstInt(0, r.ty.ty)),
                ),
                sp,
            );
            self.emit(StatementAssert(OperandCopy(place::local(c)), AssertKindDivZero), sp);
        }
        let res = self.temp(op_ty, sp);
        self.assign(place::local(res), RvalueBinary(op, OperandCopy(place::local(cur)), rhs), sp);
        let back = if lt.ty == op_ty.ty {
            RvalueUse(OperandCopy(place::local(res)))
        } else {
            RvalueCast(cast_kind(op_ty.ty, lt.ty), OperandCopy(place::local(res)), lt)
        };
        self.assign(p.clone(), back, sp);
        p
    }

    fn inc_dec(&mut self, k: IncDec, l: &texpr, sp: span) -> Operand {
        let p = self.place(l);
        let lt = l.ty.unqual();
        let old = self.temp(lt, sp);
        self.assign(place::local(old), RvalueUse(OperandCopy(p.clone())), sp);
        let inc = matches!(k, IncDecPreInc | IncDecPostInc);
        let newv = if is_pointer(lt.ty) {
            RvaluePtrAdd(
                OperandCopy(place::local(old)),
                OperandConst(ConstInt(if inc { 1 } else { -1 }, int_of(IntKindLong))),
            )
        } else {
            let one = if is_float(lt.ty) {
                OperandConst(ConstFloat(1.0, lt.ty))
            } else {
                OperandConst(ConstInt(1, lt.ty))
            };
            let op = if inc { crate::parse::ast::BinOpAdd } else { crate::parse::ast::BinOpSub };
            if matches!(kind(lt.ty), TypeKindBool) {
                // `b++` sets a bool to true; `b--` toggles it.
                if inc {
                    RvalueUse(OperandConst(ConstInt(1, lt.ty)))
                } else {
                    RvalueUnary(crate::parse::ast::UnOpNot, OperandCopy(place::local(old)))
                }
            } else {
                let pt = promote(lt.ty);
                if pt != lt.ty {
                    let w = self.temp(q(pt), sp);
                    self.assign(
                        place::local(w),
                        RvalueCast(CastKindIntToInt, OperandCopy(place::local(old)), q(pt)),
                        sp,
                    );
                    let s = self.temp(q(pt), sp);
                    self.assign(
                        place::local(s),
                        RvalueBinary(
                            op,
                            OperandCopy(place::local(w)),
                            OperandConst(ConstInt(1, pt)),
                        ),
                        sp,
                    );
                    RvalueCast(CastKindIntToInt, OperandCopy(place::local(s)), lt)
                } else {
                    RvalueBinary(op, OperandCopy(place::local(old)), one)
                }
            }
        };
        self.assign(p.clone(), newv, sp);
        if matches!(k, IncDecPostInc | IncDecPostDec) {
            OperandCopy(place::local(old))
        } else {
            OperandCopy(p)
        }
    }
}

fn cast_kind(from: type_id, to: type_id) -> CastKind {
    match (is_float(from), is_float(to)) {
        (true, true) => CastKindFloatToFloat,
        (true, false) => CastKindFloatToInt,
        (false, true) => CastKindIntToFloat,
        (false, false) => CastKindIntToInt,
    }
}

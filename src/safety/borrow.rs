/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Borrow and lifetime checking on CrIR (G-UAF, G-ALIAS). Each value that
// can hold a pointer carries a set of origins: the loans it may point into,
// the lifetime regions of borrowed parameters, static data, or unknown
// memory. A loan is live while a live local may hold it. Accesses that
// conflict with a live loan, locals that die while borrowed, and borrows
// that escape are errors.
use super::describe_place;
use crate::crir::*;
use crate::diag::{self, span};
use crate::sema::tir::program;
use crate::sema::types::*;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Origin {
    OriginLoan(u32),
    OriginRegion(u32),
    OriginStatic,
    OriginUnknown,
}
use Origin::*;

type origin_set = BTreeSet<Origin>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LoanKind {
    LoanKindShared,
    LoanKindMut,
    LoanKindRaw,
}
use LoanKind::*;

struct loan {
    place: place,
    kind: LoanKind,
    span: span,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    AccessRead,
    AccessWrite,
    AccessMove,
    AccessBorrowShared,
    AccessBorrowMut,
    AccessBorrowRaw,
    AccessDead,
}
use Access::*;

struct checker<'a> {
    b: &'a body,
    prog: &'a program,
    loans: Vec<loan>,
    loan_at: HashMap<(usize, usize), u32>,
    regions: Vec<sym_name>,
    param_region: HashMap<local, u32>,
    ret_region: Option<u32>,
    reporting: bool,
    seen: HashSet<(span, &'static str)>,
}

type sym_name = String;

#[derive(Clone, PartialEq, Eq)]
struct state {
    o: Vec<origin_set>,
}

impl state {
    fn join(&mut self, other: &state) -> bool {
        let mut changed = false;
        for (a, b) in self.o.iter_mut().zip(&other.o) {
            for x in b {
                changed |= a.insert(*x);
            }
        }
        changed
    }
}

/// Regions of a function type's borrowed parameters and of its result.
/// Returns (region per parameter, result region, names).
fn signature_regions(ft: &func_type) -> (Vec<Option<u32>>, Option<u32>, Vec<String>, bool) {
    let mut names: Vec<String> = Vec::new();
    let mut per_param = Vec::new();
    let region_of = |t: type_id, names: &mut Vec<String>, i: usize| -> Option<u32> {
        let l = match kind(t) {
            TypeKindRef { lifetime, .. } | TypeKindSlice { lifetime, .. } => lifetime,
            _ => return None,
        };
        let name = match l {
            Some(l) => l.to_string(),
            None => format!("'{}", i + 1),
        };
        if let Some(p) = names.iter().position(|n| *n == name) {
            return Some(p as u32);
        }
        names.push(name);
        Some(names.len() as u32 - 1)
    };
    for (i, p) in ft.params.iter().enumerate() {
        per_param.push(region_of(p.ty, &mut names, i));
    }
    let mut ambiguous = false;
    let ret = match kind(ft.ret.ty) {
        TypeKindRef { lifetime: Some(l), .. } | TypeKindSlice { lifetime: Some(l), .. } => {
            let n = l.to_string();
            match names.iter().position(|x| *x == n) {
                Some(p) => Some(p as u32),
                None => {
                    names.push(n);
                    Some(names.len() as u32 - 1)
                }
            }
        }
        TypeKindRef { lifetime: None, .. } | TypeKindSlice { lifetime: None, .. } => {
            let refs: Vec<u32> = per_param.iter().flatten().copied().collect();
            if refs.len() == 1 {
                Some(refs[0])
            } else {
                ambiguous = true;
                None
            }
        }
        _ => None,
    };
    (per_param, ret, names, ambiguous)
}

pub fn check(b: &body, prog: &program) {
    let gd = &prog.globals[b.func as usize];
    let ft = func_of(gd.ty.ty).unwrap();
    let (per_param, ret_region, names, ambiguous) = signature_regions(&ft);
    if ambiguous && b.crust {
        diag::error("CR0106", gd.span, format!("cannot infer the lifetime of the borrow returned by `{}`", gd.name))
            .note("a returned borrow takes the lifetime of the only borrowed parameter; this function has none or several")
            .help("name the lifetimes, for example `ref 'a int pick(ref 'a int x, ref 'a int y)`");
    }
    let mut param_region = HashMap::new();
    for (i, r) in per_param.iter().enumerate() {
        if let Some(r) = r {
            param_region.insert((i + 1) as local, *r);
        }
    }
    let mut c = checker {
        b,
        prog,
        loans: Vec::new(),
        loan_at: HashMap::new(),
        regions: names,
        param_region,
        ret_region,
        reporting: false,
        seen: HashSet::new(),
    };
    c.collect_loans();
    let states = c.origins_dataflow();
    let live = c.liveness();
    c.reporting = true;
    c.report(&states, &live);
}

impl checker<'_> {
    fn ty(&self, p: &place) -> qual_type {
        self.b.place_ty(p, self.prog)
    }

    fn once(&mut self, sp: span, code: &'static str) -> bool {
        self.reporting && self.seen.insert((sp, code))
    }

    fn name(&self, p: &place) -> String {
        describe_place(self.b, p, self.prog)
    }

    fn collect_loans(&mut self) {
        for (bi, bb) in self.b.blocks.iter().enumerate() {
            for (si, s) in bb.stmts.iter().enumerate() {
                if let StatementAssign(_, rv) = &s.kind {
                    let (p, k) = match rv {
                        RvalueRef(m, p) => (p, if *m { LoanKindMut } else { LoanKindShared }),
                        RvalueAddrOf(p) => (p, LoanKindRaw),
                        _ => continue,
                    };
                    if let PlaceBaseLocal(_) = p.base {
                        self.loan_at.insert((bi, si), self.loans.len() as u32);
                        self.loans.push(loan { place: p.clone(), kind: k, span: s.span });
                    }
                }
            }
        }
    }

    fn entry_state(&self) -> state {
        let n = self.b.locals.len();
        let mut st = state { o: vec![origin_set::new(); n] };
        for (i, l) in self.b.locals.iter().enumerate() {
            let li = i as local;
            if let Some(r) = self.param_region.get(&li) {
                st.o[i].insert(OriginRegion(*r));
            } else if (l.is_param || l.zero_init) && may_hold_pointer(l.ty.ty) {
                st.o[i].insert(OriginUnknown);
            }
        }
        st
    }

    /// Origins of the value read from a place.
    fn read_place(&self, st: &state, p: &place) -> origin_set {
        if !may_hold_pointer(self.ty(p).ty) {
            return origin_set::new();
        }
        self.memory_origins(st, p)
    }

    /// Origins of whatever is stored at a place, regardless of its type.
    fn memory_origins(&self, st: &state, p: &place) -> origin_set {
        let last_deref = p.proj.iter().rposition(|x| matches!(x, ProjDeref));
        match last_deref {
            None => match p.base {
                PlaceBaseLocal(l) => st.o[l as usize].clone(),
                PlaceBaseGlobal(_) => [OriginUnknown].into(),
                PlaceBaseString(_) => origin_set::new(),
            },
            Some(i) => {
                let prefix = place { base: p.base.clone(), proj: p.proj[..i].to_vec() };
                let ptr = self.memory_origins(st, &prefix);
                let mut out = origin_set::new();
                for o in ptr {
                    match o {
                        OriginLoan(l) => {
                            let lp = self.loans[l as usize].place.clone();
                            out.extend(self.memory_origins(st, &lp));
                        }
                        OriginRegion(r) => {
                            out.insert(OriginRegion(r));
                        }
                        OriginStatic | OriginUnknown => {
                            out.insert(OriginUnknown);
                        }
                    }
                }
                out
            }
        }
    }

    /// Origins of the pointer that the last deref in `p` goes through.
    fn deref_pointer_origins(&self, st: &state, p: &place) -> Option<origin_set> {
        let i = p.proj.iter().rposition(|x| matches!(x, ProjDeref))?;
        let prefix = place { base: p.base.clone(), proj: p.proj[..i].to_vec() };
        Some(self.memory_origins(st, &prefix))
    }

    fn operand(&self, st: &state, o: &Operand) -> origin_set {
        match o {
            OperandCopy(p) | OperandMove(p) => self.read_place(st, p),
            OperandConst(c) => match c {
                ConstGlobalAddr(_) | ConstStringAddr(_) => [OriginStatic].into(),
                ConstNull(_) => [OriginUnknown].into(),
                ConstInt(_, t) if is_pointer(*t) => [OriginUnknown].into(),
                _ => origin_set::new(),
            },
        }
    }

    fn rvalue(&self, st: &state, rv: &Rvalue, loan: Option<u32>, dest_ty: type_id) -> origin_set {
        match rv {
            RvalueUse(o) | RvalueArrayToSlice(o, _) => self.operand(st, o),
            RvalueCast(k, o, _) => match k {
                crate::sema::tir::CastKindIntToPtr => [OriginUnknown].into(),
                _ => self.operand(st, o),
            },
            RvaluePtrAdd(p, _) => {
                let mut s = self.operand(st, p);
                s.insert(OriginUnknown);
                s
            }
            RvalueAddrOf(p) | RvalueRef(_, p) => {
                let mut s: origin_set = match (loan, &p.base) {
                    (Some(l), _) => [OriginLoan(l)].into(),
                    (None, _) => [OriginStatic].into(),
                };
                if let Some(base) = self.deref_pointer_origins(st, p) {
                    s.extend(base);
                }
                s
            }
            RvalueVaArg(_) => [OriginUnknown].into(),
            RvalueZero if may_hold_pointer(dest_ty) => [OriginUnknown].into(),
            _ => origin_set::new(),
        }
    }

    /// Records a store of a value with origins `o` into `p`.
    fn write(&mut self, st: &mut state, p: &place, o: origin_set, sp: span, checked: bool) {
        let has_local_loan = o
            .iter()
            .any(|x| matches!(x, OriginLoan(l) if !self.loans[*l as usize].place.has_deref()));
        match (p.has_deref(), &p.base) {
            (false, PlaceBaseLocal(l)) => {
                if p.proj.is_empty() {
                    st.o[*l as usize] = o;
                } else {
                    st.o[*l as usize].extend(o);
                }
            }
            (false, _) => {
                if has_local_loan && self.once(sp, "CR0521") {
                    let l = o.iter().find_map(|x| match x {
                        OriginLoan(l) => Some(*l),
                        _ => None,
                    });
                    let mut d = diag::error("CR0521", sp, format!("a pointer to a local escapes into `{}`", self.name(p)))
                        .label("the local is gone when the function returns, but the global keeps pointing at it");
                    if let Some(l) = l {
                        d = d.secondary(self.loans[l as usize].span, "borrowed here");
                    }
                    drop(d);
                }
            }
            (true, _) => {
                let ptr = self.deref_pointer_origins(st, p).unwrap_or_default();
                let mut targets = Vec::new();
                for x in &ptr {
                    if let OriginLoan(l) = x {
                        let lp = &self.loans[*l as usize].place;
                        if let (false, PlaceBaseLocal(t)) = (lp.has_deref(), &lp.base) {
                            targets.push(*t);
                        }
                    }
                }
                for t in targets {
                    st.o[t as usize].extend(o.iter().copied());
                }
                let escapes =
                    ptr.iter().any(|x| matches!(x, OriginUnknown | OriginStatic | OriginRegion(_)));
                if checked && escapes && has_local_loan && self.once(sp, "CR0521") {
                    diag::error("CR0521", sp, "a borrow of a local is stored where the checker cannot follow it")
                        .label("stored through a pointer whose target is not tracked")
                        .help("keep the borrow in a local variable, or do this inside `unsafe` with a `SAFETY:` comment");
                }
                if checked {
                    let dst_regions: Vec<u32> = ptr
                        .iter()
                        .filter_map(|x| if let OriginRegion(r) = x { Some(*r) } else { None })
                        .collect();
                    for x in &o {
                        if let OriginRegion(r) = x {
                            if !dst_regions.is_empty()
                                && !dst_regions.contains(r)
                                && self.once(sp, "CR0623")
                            {
                                diag::error(
                                    "CR0623",
                                    sp,
                                    format!(
                                        "lifetime mismatch: data with lifetime `{}` is stored into a borrow with lifetime `{}`",
                                        self.regions[*r as usize],
                                        self.regions[dst_regions[0] as usize]
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn call_effects(&mut self, st: &mut state, s: &statement) {
        let StatementCall { dest, func, args, .. } = &s.kind else { return };
        let fty = match func {
            OperandConst(ConstGlobalAddr(g)) => self.prog.globals[*g as usize].ty.ty,
            OperandCopy(p) | OperandMove(p) => self.ty(p).ty,
            _ => return,
        };
        let Some(ft) = func_of(fty) else { return };
        let (per_param, ret_region, _, ambiguous) = signature_regions(&ft);
        let arg_o: Vec<origin_set> = args.iter().map(|a| self.operand(st, a)).collect();
        for (i, a) in args.iter().enumerate() {
            let at = match a {
                OperandCopy(p) | OperandMove(p) => self.ty(p),
                _ => continue,
            };
            let writable = match at.kind() {
                TypeKindPointer(p) => !p.is_const() && may_hold_pointer(p.ty),
                TypeKindRef { mutable: true, pointee, .. } => may_hold_pointer(pointee.ty),
                _ => false,
            };
            if !writable {
                continue;
            }
            let raw = is_pointer(at.ty);
            // What the callee could store through this argument: the other
            // arguments. A value cannot hold a borrow of itself.
            let mut others = origin_set::new();
            for (j, o) in arg_o.iter().enumerate() {
                if j != i {
                    others.extend(o.iter().copied());
                }
            }
            for x in arg_o[i].clone() {
                if let OriginLoan(l) = x {
                    let lp = self.loans[l as usize].place.clone();
                    if let (false, PlaceBaseLocal(t)) = (lp.has_deref(), &lp.base) {
                        st.o[*t as usize].extend(others.iter().copied());
                        if raw {
                            st.o[*t as usize].insert(OriginUnknown);
                        }
                    }
                }
            }
        }
        if let Some(d) = dest {
            let dt = self.ty(d).ty;
            let o = if is_ref(ft.ret.ty) || is_slice(ft.ret.ty) {
                let mut o = origin_set::new();
                for (i, r) in per_param.iter().enumerate() {
                    if ambiguous || (r.is_some() && *r == ret_region) {
                        if let Some(x) = arg_o.get(i) {
                            o.extend(x.iter().copied());
                        }
                    }
                }
                o
            } else if may_hold_pointer(dt) {
                [OriginUnknown].into()
            } else {
                origin_set::new()
            };
            self.write(st, d, o, s.span, !s.unchecked);
        }
    }

    fn transfer(&mut self, st: &mut state, bi: usize, si: usize, s: &statement) {
        match &s.kind {
            StatementAssign(p, rv) => {
                let loan = self.loan_at.get(&(bi, si)).copied();
                let dt = self.ty(p).ty;
                let o = self.rvalue(st, rv, loan, dt);
                self.write(st, p, o, s.span, !s.unchecked);
            }
            StatementCall { .. } => self.call_effects(st, s),
            StatementStorageDead(l) | StatementStorageLive(l) => st.o[*l as usize].clear(),
            _ => {}
        }
    }

    fn origins_dataflow(&mut self) -> Vec<state> {
        let n = self.b.blocks.len();
        let empty = state { o: vec![origin_set::new(); self.b.locals.len()] };
        let mut states = vec![empty; n];
        states[0] = self.entry_state();
        let mut work: Vec<usize> = (0..n).rev().collect();
        let mut queued = vec![true; n];
        let mut rounds = 0usize;
        while let Some(i) = work.pop() {
            queued[i] = false;
            rounds += 1;
            if rounds > 200_000 {
                break;
            }
            let mut st = states[i].clone();
            let stmts = self.b.blocks[i].stmts.clone();
            for (si, s) in stmts.iter().enumerate() {
                self.transfer(&mut st, i, si, s);
            }
            for succ in self.b.blocks[i].term.successors() {
                let si = succ as usize;
                if states[si].join(&st) && !queued[si] {
                    queued[si] = true;
                    work.push(si);
                }
            }
        }
        states
    }

    // ---- liveness ----

    fn uses_defs(&self, s: &StatementKind) -> (Vec<local>, Option<local>) {
        let mut uses = Vec::new();
        let mut def = None;
        let mut place_uses = |p: &place, uses: &mut Vec<local>| {
            if let PlaceBaseLocal(l) = p.base {
                uses.push(l);
            }
            for pr in &p.proj {
                if let ProjIndex(i) = pr {
                    uses.push(*i);
                }
            }
        };
        let op =
            |o: &Operand, uses: &mut Vec<local>, pu: &mut dyn FnMut(&place, &mut Vec<local>)| {
                if let Some(p) = o.place() {
                    pu(p, uses);
                }
            };
        match s {
            StatementAssign(p, rv) => {
                match rv {
                    RvalueUse(o)
                    | RvalueUnary(_, o)
                    | RvalueCast(_, o, _)
                    | RvalueArrayToSlice(o, _)
                    | RvalueVaArg(o) => op(o, &mut uses, &mut place_uses),
                    RvalueBinary(_, a, c) | RvaluePtrAdd(a, c) | RvaluePtrDiff(a, c) => {
                        op(a, &mut uses, &mut place_uses);
                        op(c, &mut uses, &mut place_uses);
                    }
                    RvalueAddrOf(x) | RvalueRef(_, x) | RvalueSliceLen(x) => {
                        place_uses(x, &mut uses)
                    }
                    RvalueZero => {}
                }
                match p.as_local() {
                    Some(l) => def = Some(l),
                    None => place_uses(p, &mut uses),
                }
            }
            StatementCall { dest, func, args, .. } => {
                op(func, &mut uses, &mut place_uses);
                for a in args {
                    op(a, &mut uses, &mut place_uses);
                }
                if let Some(d) = dest {
                    match d.as_local() {
                        Some(l) => def = Some(l),
                        None => place_uses(d, &mut uses),
                    }
                }
            }
            StatementDrop(p) | StatementDropIfInit(p) => place_uses(p, &mut uses),
            StatementDropIfFlag(p, f) => {
                place_uses(p, &mut uses);
                uses.push(*f);
            }
            StatementAssert(o, _) | StatementVaStart(o) | StatementVaEnd(o) => {
                op(o, &mut uses, &mut place_uses)
            }
            StatementVaCopy(a, c) => {
                op(a, &mut uses, &mut place_uses);
                op(c, &mut uses, &mut place_uses);
            }
            StatementStorageDead(l) => def = Some(*l),
            _ => {}
        }
        (uses, def)
    }

    /// Live locals before each statement, per block. The extra last entry
    /// is the state before the terminator.
    fn liveness(&self) -> Vec<Vec<HashSet<local>>> {
        let n = self.b.blocks.len();
        let mut live_in: Vec<HashSet<local>> = vec![HashSet::new(); n];
        let mut changed = true;
        let mut out = vec![Vec::new(); n];
        while changed {
            changed = false;
            for i in (0..n).rev() {
                let bb = &self.b.blocks[i];
                let mut live: HashSet<local> = HashSet::new();
                for s in bb.term.successors() {
                    live.extend(live_in[s as usize].iter().copied());
                }
                if let TermSwitch(o, ..) = &bb.term {
                    if let Some(p) = o.place() {
                        if let PlaceBaseLocal(l) = p.base {
                            live.insert(l);
                        }
                    }
                }
                if let TermReturn = bb.term {
                    live.insert(ReturnSlot);
                }
                let mut per = vec![live.clone()];
                for s in bb.stmts.iter().rev() {
                    let (uses, def) = self.uses_defs(&s.kind);
                    if let Some(d) = def {
                        live.remove(&d);
                    }
                    live.extend(uses);
                    per.push(live.clone());
                }
                per.reverse();
                if live != live_in[i] {
                    live_in[i] = live;
                    changed = true;
                }
                out[i] = per;
            }
        }
        out
    }

    // ---- reporting ----

    fn live_loans(&self, st: &state, live: &HashSet<local>) -> Vec<(u32, local)> {
        let mut out: Vec<(u32, local)> = Vec::new();
        let mut seen = HashSet::new();
        let mut stack: Vec<(u32, local)> = Vec::new();
        for &l in live {
            for o in &st.o[l as usize] {
                if let OriginLoan(x) = o {
                    stack.push((*x, l));
                }
            }
        }
        while let Some((x, holder)) = stack.pop() {
            if !seen.insert(x) {
                continue;
            }
            out.push((x, holder));
            let lp = &self.loans[x as usize].place;
            if let (false, PlaceBaseLocal(t)) = (lp.has_deref(), &lp.base) {
                if may_hold_pointer(self.b.locals[*t as usize].ty.ty) {
                    for o in &st.o[*t as usize] {
                        if let OriginLoan(y) = o {
                            stack.push((*y, holder));
                        }
                    }
                }
            }
        }
        out
    }

    fn accesses(&self, s: &StatementKind) -> Vec<(place, Access)> {
        let mut v = Vec::new();
        let op = |o: &Operand, v: &mut Vec<(place, Access)>| match o {
            OperandCopy(p) => v.push((p.clone(), AccessRead)),
            OperandMove(p) => v.push((p.clone(), AccessMove)),
            _ => {}
        };
        match s {
            StatementAssign(p, rv) => {
                match rv {
                    RvalueUse(o)
                    | RvalueUnary(_, o)
                    | RvalueCast(_, o, _)
                    | RvalueArrayToSlice(o, _)
                    | RvalueVaArg(o) => op(o, &mut v),
                    RvalueBinary(_, a, c) | RvaluePtrAdd(a, c) | RvaluePtrDiff(a, c) => {
                        op(a, &mut v);
                        op(c, &mut v);
                    }
                    RvalueRef(false, x) => v.push((x.clone(), AccessBorrowShared)),
                    RvalueRef(true, x) => v.push((x.clone(), AccessBorrowMut)),
                    RvalueAddrOf(x) => v.push((x.clone(), AccessBorrowRaw)),
                    RvalueSliceLen(x) => v.push((x.clone(), AccessRead)),
                    RvalueZero => {}
                }
                v.push((p.clone(), AccessWrite));
            }
            StatementCall { dest, func, args, .. } => {
                op(func, &mut v);
                for a in args {
                    op(a, &mut v);
                }
                if let Some(d) = dest {
                    v.push((d.clone(), AccessWrite));
                }
            }
            StatementDrop(p) | StatementDropIfFlag(p, _) => v.push((p.clone(), AccessMove)),
            StatementAssert(o, _) | StatementVaStart(o) | StatementVaEnd(o) => op(o, &mut v),
            StatementVaCopy(a, c) => {
                op(a, &mut v);
                op(c, &mut v);
            }
            StatementStorageDead(l) => v.push((place::local(*l), AccessDead)),
            _ => {}
        }
        v
    }

    /// Whether an access to `a` touches the memory of a loan on `l`.
    fn overlaps(l: &place, a: &place) -> bool {
        if l.base != a.base {
            return false;
        }
        let n = l.proj.len().min(a.proj.len());
        for i in 0..n {
            match (&l.proj[i], &a.proj[i]) {
                (ProjField(r1, f1), ProjField(r2, f2)) => {
                    if r1 == r2 && f1 != f2 && !record(*r1).is_union {
                        return false;
                    }
                }
                (ProjConstIndex(x), ProjConstIndex(y)) if x != y => return false,
                _ => {}
            }
        }
        // A loan that reaches through a pointer stored in `a` is not
        // affected by writing, moving, or ending `a` itself.
        if l.proj.len() > a.proj.len()
            && l.proj[a.proj.len()..].iter().any(|p| matches!(p, ProjDeref))
        {
            return false;
        }
        true
    }

    fn conflict(&mut self, acc: Access, a: &place, lid: u32, holder: local, sp: span) {
        let lk = self.loans[lid as usize].kind;
        let lsp = self.loans[lid as usize].span;
        let code = match (acc, lk) {
            (AccessDead, _) => "CR0597",
            (AccessMove, _) => "CR0505",
            (_, LoanKindRaw) => return,
            (AccessRead, LoanKindMut) => "CR0502",
            (AccessRead, _) => return,
            (AccessWrite, _) => "CR0506",
            (AccessBorrowShared, LoanKindMut) => "CR0502",
            (AccessBorrowShared, _) => return,
            (AccessBorrowMut | AccessBorrowRaw, LoanKindMut) => "CR0499",
            (AccessBorrowMut | AccessBorrowRaw, LoanKindShared) => "CR0502",
        };
        let code = if code == "CR0597" && holder == ReturnSlot { "CR0515" } else { code };
        if !self.once(sp, code) {
            return;
        }
        let n = self.name(&self.loans[lid as usize].place);
        let an = self.name(a);
        let what = match lk {
            LoanKindShared => "borrowed",
            LoanKindMut => "mutably borrowed",
            LoanKindRaw => "pointed to",
        };
        let msg = match code {
            "CR0597" => format!("`{n}` does not live long enough"),
            "CR0515" => format!("returns a reference to the local `{n}`"),
            "CR0505" => format!("cannot move out of `{an}` because it is {what}"),
            "CR0506" => format!("cannot assign to `{an}` because it is {what}"),
            "CR0499" => format!("cannot borrow `{an}` as mutable more than once at a time"),
            _ => match acc {
                AccessRead => format!("cannot use `{an}` because it is mutably borrowed"),
                AccessBorrowShared => {
                    format!("cannot borrow `{an}` because it is also borrowed as mutable")
                }
                _ => {
                    format!("cannot borrow `{an}` as mutable because it is also borrowed as shared")
                }
            },
        };
        let label = match code {
            "CR0597" => format!("`{n}` goes out of scope here while still {what}"),
            "CR0515" => "the local is gone after the return".to_string(),
            _ => "conflicting access here".to_string(),
        };
        let hl = self.b.locals[holder as usize]
            .name
            .map(|h| format!(", and `{h}` uses it later"))
            .unwrap_or_default();
        diag::error(code, sp, msg).label(label).secondary(lsp, format!("`{n}` is {what} here{hl}"));
    }

    fn check_deref(&mut self, st: &state, p: &place, sp: span) {
        let mut prefix = place { base: p.base.clone(), proj: vec![] };
        for pr in &p.proj {
            if let ProjDeref = pr {
                let t = self.ty(&prefix);
                if is_pointer(t.ty) {
                    let o = self.memory_origins(st, &prefix);
                    let tracked = !o.is_empty() && !o.contains(&OriginUnknown);
                    if !tracked && self.once(sp, "CR0133") {
                        diag::error("CR0133", sp, format!("dereferencing `{}` is unsafe: the checker cannot tell where it points", self.name(&prefix)))
                            .note("only pointers made from `&` of a live local, a string literal, a global, or a borrow are tracked")
                            .help("use a borrow (`ref`) or a slice, or wrap the access in `unsafe` with a `SAFETY:` comment");
                    }
                }
            }
            prefix.proj.push(pr.clone());
        }
    }

    fn report(&mut self, states: &[state], live: &[Vec<HashSet<local>>]) {
        let preds = self.b.predecessors();
        for bi in 0..self.b.blocks.len() {
            if bi != 0 && preds[bi].is_empty() {
                continue;
            }
            let mut st = states[bi].clone();
            let stmts = self.b.blocks[bi].stmts.clone();
            for (si, s) in stmts.iter().enumerate() {
                let lv = &live[bi][si];
                let loans = self.live_loans(&st, lv);
                for (a, acc) in self.accesses(&s.kind) {
                    if self.b.crust && !s.unchecked && acc != AccessDead {
                        self.check_deref(&st, &a, s.span);
                    }
                    for &(l, holder) in &loans {
                        if Self::overlaps(&self.loans[l as usize].place, &a) {
                            self.conflict(acc, &a, l, holder, s.span);
                        }
                    }
                }
                self.transfer(&mut st, bi, si, s);
            }
            if let TermReturn = self.b.blocks[bi].term {
                self.check_return(&st, self.b.blocks[bi].term_span);
            }
        }
    }

    fn check_return(&mut self, st: &state, sp: span) {
        let ret = self.b.locals[ReturnSlot as usize].ty;
        if !may_hold_pointer(ret.ty) {
            return;
        }
        for o in st.o[ReturnSlot as usize].clone() {
            match o {
                OriginLoan(l) => {
                    let lp = self.loans[l as usize].place.clone();
                    if !lp.has_deref() && self.once(sp, "CR0515") {
                        diag::error(
                            "CR0515",
                            sp,
                            format!("returns a reference to the local `{}`", self.name(&lp)),
                        )
                        .secondary(self.loans[l as usize].span, "borrowed here")
                        .note("the local is gone when the function returns");
                    }
                }
                OriginRegion(r)
                    if self.b.crust
                        && self.ret_region.is_some()
                        && Some(r) != self.ret_region
                        && (is_ref(ret.ty) || is_slice(ret.ty))
                        && self.once(sp, "CR0623") =>
                {
                    let want = self
                        .ret_region
                        .map(|x| self.regions[x as usize].clone())
                        .unwrap_or_else(|| "?".into());
                    diag::error(
                            "CR0623",
                            sp,
                            format!("lifetime mismatch: returns data with lifetime `{}`, but the result has lifetime `{want}`", self.regions[r as usize]),
                        )
                        .help("give the parameter and the result the same lifetime name");
                }
                _ => {}
            }
        }
    }
}

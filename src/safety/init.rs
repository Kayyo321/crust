/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Move and definite-initialization analysis (G-UNINIT, G-UAF, G-DF), and
// drop elaboration (G-LEAK): every owned value is dropped on every path,
// exactly once.
use super::bitset::bitset;
use super::describe_place;
use crate::crir::*;
use crate::diag::{self, span};
use crate::sema::tir::program;
use crate::sema::types::*;
use std::collections::{HashMap, HashSet};

pub struct move_path {
    pub place: place,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub ty: qual_type,
}

pub struct move_data {
    pub paths: Vec<move_path>,
    map: HashMap<place, usize>,
    /// Bit indices of the leaves under each path.
    pub leaves: Vec<Vec<usize>>,
    pub nbits: usize,
}

/// Initialization state at a point: which leaves may be initialized, may be
/// uninitialized, and may have been moved out.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct init_state {
    pub init: bitset,
    pub uninit: bitset,
    pub moved: bitset,
}

impl init_state {
    fn bottom(n: usize) -> init_state {
        init_state { init: bitset::new(n), uninit: bitset::new(n), moved: bitset::new(n) }
    }

    fn join(&mut self, o: &init_state) -> bool {
        let a = self.init.union(&o.init);
        let b = self.uninit.union(&o.uninit);
        let c = self.moved.union(&o.moved);
        a || b || c
    }
}

/// The move-path prefix of a place: its local and leading field projections.
fn movable_prefix(p: &place) -> Option<place> {
    let PlaceBaseLocal(l) = p.base else { return None };
    let mut out = place::local(l);
    for pr in &p.proj {
        match pr {
            ProjField(r, _) if !record(*r).is_union => out.proj.push(pr.clone()),
            _ => break,
        }
    }
    Some(out)
}

fn for_each_place(s: &StatementKind, mut f: impl FnMut(&place, bool)) {
    let op = |o: &Operand, f: &mut dyn FnMut(&place, bool)| {
        if let Some(p) = o.place() {
            f(p, false);
        }
    };
    match s {
        StatementAssign(p, rv) => {
            match rv {
                RvalueUse(o)
                | RvalueUnary(_, o)
                | RvalueCast(_, o, _)
                | RvalueArrayToSlice(o, _)
                | RvalueVaArg(o) => op(o, &mut f),
                RvalueBinary(_, a, b) | RvaluePtrAdd(a, b) | RvaluePtrDiff(a, b) => {
                    op(a, &mut f);
                    op(b, &mut f);
                }
                RvalueAddrOf(x) | RvalueRef(_, x) | RvalueSliceLen(x) => f(x, false),
                RvalueZero => {}
            }
            f(p, true);
        }
        StatementCall { dest, func, args, .. } => {
            op(func, &mut f);
            for a in args {
                op(a, &mut f);
            }
            if let Some(d) = dest {
                f(d, true);
            }
        }
        StatementDropIfInit(p) | StatementDrop(p) | StatementDropIfFlag(p, _) => f(p, false),
        StatementAssert(o, _) | StatementVaStart(o) | StatementVaEnd(o) => op(o, &mut f),
        StatementVaCopy(a, b) => {
            op(a, &mut f);
            op(b, &mut f);
        }
        _ => {}
    }
}

impl move_data {
    pub fn build(b: &body, _prog: &program) -> move_data {
        let mut md =
            move_data { paths: Vec::new(), map: HashMap::new(), leaves: Vec::new(), nbits: 0 };
        for (i, l) in b.locals.iter().enumerate() {
            let p = place::local(i as local);
            md.map.insert(p.clone(), md.paths.len());
            md.paths.push(move_path { place: p, parent: None, children: vec![], ty: l.ty });
        }
        for bb in &b.blocks {
            for s in &bb.stmts {
                for_each_place(&s.kind, |p, _| {
                    if let Some(pre) = movable_prefix(p) {
                        md.ensure(&pre);
                    }
                });
            }
        }
        // Leaves get the bits.
        let n = md.paths.len();
        let mut bit_of = vec![usize::MAX; n];
        for i in 0..n {
            if md.paths[i].children.is_empty() {
                bit_of[i] = md.nbits;
                md.nbits += 1;
            }
        }
        md.leaves = vec![Vec::new(); n];
        for i in 0..n {
            let mut stack = vec![i];
            while let Some(x) = stack.pop() {
                if md.paths[x].children.is_empty() {
                    md.leaves[i].push(bit_of[x]);
                } else {
                    stack.extend(md.paths[x].children.iter().copied());
                }
            }
        }
        md
    }

    fn ensure(&mut self, p: &place) -> usize {
        if let Some(&i) = self.map.get(p) {
            return i;
        }
        let mut parent_place = p.clone();
        let last = parent_place.proj.pop().unwrap();
        let parent = self.ensure(&parent_place);
        let ProjField(r, _) = last else { unreachable!() };
        // Create every field of the record, so the children partition it.
        let fields = record(r).fields.clone();
        for (fi, f) in fields.iter().enumerate() {
            let cp = parent_place.project(ProjField(r, fi as u32));
            if self.map.contains_key(&cp) {
                continue;
            }
            let idx = self.paths.len();
            self.map.insert(cp.clone(), idx);
            self.paths.push(move_path {
                place: cp,
                parent: Some(parent),
                children: vec![],
                ty: f.ty,
            });
            self.paths[parent].children.push(idx);
        }
        self.map[p]
    }

    /// The deepest move path that is a prefix of the place.
    pub fn path_of(&self, p: &place) -> Option<usize> {
        let pre = movable_prefix(p)?;
        let mut cur = pre;
        loop {
            if let Some(&i) = self.map.get(&cur) {
                return Some(i);
            }
            cur.proj.pop()?;
        }
    }

    fn exact_path(&self, p: &place) -> Option<usize> {
        let pre = movable_prefix(p)?;
        if pre.proj.len() != p.proj.len() {
            return None;
        }
        self.map.get(&pre).copied()
    }

    fn do_init(&self, st: &mut init_state, path: usize) {
        for &l in &self.leaves[path] {
            st.init.set(l);
            st.uninit.clear(l);
            st.moved.clear(l);
        }
    }

    fn do_move(&self, st: &mut init_state, path: usize) {
        for &l in &self.leaves[path] {
            st.init.clear(l);
            st.uninit.set(l);
            st.moved.set(l);
        }
    }

    fn do_uninit(&self, st: &mut init_state, path: usize) {
        for &l in &self.leaves[path] {
            st.init.clear(l);
            st.uninit.set(l);
            st.moved.clear(l);
        }
    }

    pub fn maybe_uninit(&self, st: &init_state, path: usize) -> bool {
        self.leaves[path].iter().any(|&l| st.uninit.get(l))
    }

    pub fn maybe_init(&self, st: &init_state, path: usize) -> bool {
        self.leaves[path].iter().any(|&l| st.init.get(l))
    }

    fn maybe_moved(&self, st: &init_state, path: usize) -> bool {
        self.leaves[path].iter().any(|&l| st.moved.get(l))
    }
}

/// Locals whose address is taken, or that are written element by element,
/// are zero-filled at entry. They can never be read uninitialized, and C
/// idioms such as `scanf("%d", &x)` keep working (D-037).
pub fn mark_zero_init(b: &mut body, _prog: &program) {
    let mut marks = HashSet::new();
    for bb in &b.blocks {
        for s in &bb.stmts {
            // A write below the move-path level (an element, or a union
            // member) that does not go through a pointer.
            let indexed_write = |p: &place| -> Option<local> {
                let l = p.root_local()?;
                let pre = movable_prefix(p)?;
                let rest = &p.proj[pre.proj.len()..];
                (!rest.is_empty() && !matches!(rest[0], ProjDeref)).then_some(l)
            };
            match &s.kind {
                StatementAssign(p, rv) => {
                    if let RvalueAddrOf(x) = rv {
                        if !x.proj.iter().any(|pr| matches!(pr, ProjDeref)) {
                            if let Some(l) = x.root_local() {
                                marks.insert(l);
                            }
                        }
                    }
                    if let Some(l) = indexed_write(p) {
                        marks.insert(l);
                    }
                }
                StatementCall { dest: Some(p), .. } => {
                    if let Some(l) = indexed_write(p) {
                        marks.insert(l);
                    }
                }
                _ => {}
            }
        }
    }
    for l in marks {
        let ld = &mut b.locals[l as usize];
        if !is_owned(ld.ty.ty) && !ld.is_param && l != ReturnSlot {
            ld.zero_init = true;
        }
    }
}

fn entry_state(b: &body, md: &move_data) -> init_state {
    let mut st = init_state::bottom(md.nbits);
    for (i, l) in b.locals.iter().enumerate() {
        if l.is_param || l.zero_init {
            md.do_init(&mut st, i);
        } else {
            md.do_uninit(&mut st, i);
        }
        let _ = i;
    }
    st
}

fn transfer(md: &move_data, b: &body, st: &mut init_state, s: &statement) {
    let move_op = |st: &mut init_state, o: &Operand| {
        if let OperandMove(p) = o {
            if let Some(path) = md.exact_path(p) {
                md.do_move(st, path);
            }
        }
    };
    match &s.kind {
        StatementAssign(p, rv) => {
            match rv {
                RvalueUse(o) | RvalueCast(_, o, _) => move_op(st, o),
                _ => {}
            }
            if let Some(path) = md.exact_path(p) {
                md.do_init(st, path);
            }
        }
        StatementCall { dest, args, .. } => {
            for a in args {
                move_op(st, a);
            }
            if let Some(d) = dest {
                if let Some(path) = md.exact_path(d) {
                    md.do_init(st, path);
                }
            }
        }
        StatementDropIfInit(p) | StatementDrop(p) | StatementDropIfFlag(p, _) => {
            if let Some(path) = md.exact_path(p) {
                md.do_move(st, path);
            }
        }
        StatementStorageDead(l) => md.do_uninit(st, *l as usize),
        StatementStorageLive(l) => {
            if b.locals[*l as usize].zero_init {
                md.do_init(st, *l as usize);
            } else {
                md.do_uninit(st, *l as usize);
            }
        }
        _ => {}
    }
}

/// Runs the forward dataflow. Returns the state at entry to each block.
pub fn analyze(b: &body, _prog: &program, md: &move_data) -> Vec<init_state> {
    let n = b.blocks.len();
    let mut states = vec![init_state::bottom(md.nbits); n];
    states[0] = entry_state(b, md);
    let mut work: Vec<usize> = (0..n).rev().collect();
    let mut queued = vec![true; n];
    while let Some(i) = work.pop() {
        queued[i] = false;
        let mut st = states[i].clone();
        for s in &b.blocks[i].stmts {
            transfer(md, b, &mut st, s);
        }
        for succ in b.blocks[i].term.successors() {
            let si = succ as usize;
            if states[si].join(&st) && !queued[si] {
                queued[si] = true;
                work.push(si);
            }
        }
    }
    states
}

struct reporter<'a> {
    b: &'a body,
    prog: &'a program,
    md: &'a move_data,
    seen: HashSet<(span, &'static str)>,
    move_sites: HashMap<usize, span>,
}

impl reporter<'_> {
    fn once(&mut self, sp: span, code: &'static str) -> bool {
        self.seen.insert((sp, code))
    }

    fn name(&self, p: &place) -> String {
        describe_place(self.b, p, self.prog)
    }

    fn check_read(&mut self, st: &init_state, p: &place, sp: span, what: &str) {
        let Some(path) = self.md.path_of(p) else { return };
        if !self.md.maybe_uninit(st, path) {
            return;
        }
        let pp = self.md.paths[path].place.clone();
        if self.b.locals[pp.root_local().unwrap() as usize].name.is_none()
            && pp.root_local() != Some(ReturnSlot)
        {
            return;
        }
        if self.md.maybe_moved(st, path) {
            if !self.once(sp, "CR0382") {
                return;
            }
            let mut d =
                diag::error("CR0382", sp, format!("{what} of moved value: `{}`", self.name(&pp)))
                    .label("value used here after move");
            if let Some(ms) = self.move_sites.get(&path).copied().or_else(|| {
                self.md.leaves[path]
                    .iter()
                    .find_map(|l| self.move_sites.get(&(usize::MAX - l)).copied())
            }) {
                d = d.secondary(ms, "value moved here");
            }
            if self.b.crust {
                d = d.note(
                    "an owned value has one owner; after a move the old binding cannot be used",
                );
            }
            drop(d);
        } else {
            if !self.once(sp, "CR0381") {
                return;
            }
            let all = !self.md.maybe_init(st, path);
            let adj = if all { "isn't" } else { "may not be" };
            let mut d =
                diag::error("CR0381", sp, format!("`{}` {adj} initialized here", self.name(&pp)))
                    .label(format!("`{}` used here", self.name(&pp)));
            if let PlaceBaseLocal(l) = pp.base {
                let ld = &self.b.locals[l as usize];
                if ld.user {
                    d = d.secondary(ld.span, "declared here");
                }
            }
            drop(d.help("assign a value on every path before this use"));
        }
    }

    fn check_move_out(&mut self, p: &place, sp: span) {
        let movable = matches!(p.base, PlaceBaseLocal(_))
            && !p.proj.iter().any(|pr| !matches!(pr, ProjField(..)));
        if !movable {
            if self.once(sp, "CR0507") {
                diag::error("CR0507", sp, format!("cannot move out of `{}`", self.name(p)))
                    .label("the owned value would be moved out of a borrow, pointer, element, or global")
                    .help("move a whole local variable, or call a function that takes the value by pointer");
            }
            return;
        }
        // Moving a field out of a struct with a drop function is not allowed.
        let mut prefix = place { base: p.base.clone(), proj: vec![] };
        for pr in &p.proj {
            let t = self.b.place_ty(&prefix, self.prog);
            if let TypeKindRecord(r) = t.kind() {
                if record(r).drop.is_some()
                    || record(r).owned && with_record(r, |rd| rd.drop.is_some())
                {
                    if self.once(sp, "CR0509") {
                        diag::error(
                            "CR0509",
                            sp,
                            format!(
                                "cannot move out of `{}`, a field of a type with a drop function",
                                self.name(p)
                            ),
                        )
                        .label("move occurs here");
                    }
                    return;
                }
            }
            prefix.proj.push(pr.clone());
        }
    }

    fn check_statement(&mut self, st: &init_state, s: &statement) {
        let sp = s.span;
        let mut reads: Vec<(place, bool)> = Vec::new();
        let mut write: Option<place> = None;
        let mut borrows: Vec<(place, bool)> = Vec::new();
        match &s.kind {
            StatementAssign(p, rv) => {
                match rv {
                    RvalueUse(o)
                    | RvalueUnary(_, o)
                    | RvalueCast(_, o, _)
                    | RvalueArrayToSlice(o, _)
                    | RvalueVaArg(o) => {
                        if let Some(x) = o.place() {
                            reads.push((x.clone(), matches!(o, OperandMove(_))));
                        }
                    }
                    RvalueBinary(_, a, c) | RvaluePtrAdd(a, c) | RvaluePtrDiff(a, c) => {
                        for o in [a, c] {
                            if let Some(x) = o.place() {
                                reads.push((x.clone(), false));
                            }
                        }
                    }
                    RvalueRef(_, x) => borrows.push((x.clone(), true)),
                    RvalueAddrOf(x) => borrows.push((x.clone(), false)),
                    RvalueSliceLen(x) => reads.push((x.clone(), false)),
                    RvalueZero => {}
                }
                write = Some(p.clone());
            }
            StatementCall { dest, func, args, .. } => {
                for o in std::iter::once(func).chain(args.iter()) {
                    if let Some(x) = o.place() {
                        reads.push((x.clone(), matches!(o, OperandMove(_))));
                    }
                }
                write = dest.clone();
            }
            StatementAssert(o, _) | StatementVaStart(o) | StatementVaEnd(o) => {
                if let Some(x) = o.place() {
                    reads.push((x.clone(), false));
                }
            }
            StatementVaCopy(a, c) => {
                for o in [a, c] {
                    if let Some(x) = o.place() {
                        reads.push((x.clone(), false));
                    }
                }
            }
            _ => {}
        }
        for (p, is_move) in &reads {
            self.check_place_bases(st, p, sp);
            if *is_move {
                self.check_read(st, p, sp, "use");
                self.check_move_out(p, sp);
            } else {
                self.check_read(st, p, sp, "use");
            }
        }
        for (p, is_ref) in &borrows {
            self.check_place_bases(st, p, sp);
            let owned = is_owned(self.b.place_ty(p, self.prog).ty);
            if *is_ref || owned {
                self.check_read(st, p, sp, "borrow");
            }
        }
        if let Some(p) = write {
            self.check_place_bases(st, &p, sp);
            self.check_write(st, &p, sp);
        }
    }

    /// Reading the pointer of a deref, or the base of an index, needs it initialized.
    fn check_place_bases(&mut self, st: &init_state, p: &place, sp: span) {
        let mut prefix = place { base: p.base.clone(), proj: vec![] };
        for pr in &p.proj {
            match pr {
                ProjDeref => self.check_read(st, &prefix, sp, "use"),
                ProjIndex(l) => {
                    self.check_read(st, &place::local(*l), sp, "use");
                    self.check_read(st, &prefix, sp, "use");
                }
                ProjConstIndex(_) => self.check_read(st, &prefix, sp, "use"),
                _ => {}
            }
            prefix.proj.push(pr.clone());
        }
    }

    fn check_write(&mut self, st: &init_state, p: &place, sp: span) {
        let Some(path) = self.md.exact_path(p) else { return };
        let root = p.root_local().unwrap();
        let ld = &self.b.locals[root as usize];
        if ld.is_let && !ld.mutable && self.md.maybe_init(st, path) && ld.name.is_some() {
            if self.once(sp, "CR0384") {
                diag::error(
                    "CR0384",
                    sp,
                    format!("cannot assign twice to immutable binding `{}`", ld.name.unwrap()),
                )
                .secondary(ld.span, "declared here without `mut`")
                .help(format!("make the binding mutable: `let mut {}`", ld.name.unwrap()));
            }
        }
        // An owned value cannot be built field by field.
        if !p.proj.is_empty() && is_owned(ld.ty.ty) {
            let mine: HashSet<usize> = self.md.leaves[path].iter().copied().collect();
            let others_uninit = self.md.leaves[root as usize]
                .iter()
                .any(|l| !mine.contains(l) && st.uninit.get(*l));
            if others_uninit
                && !st.moved.get(*self.md.leaves[root as usize].first().unwrap())
                && ld.name.is_some()
            {
                let rp = place::local(root);
                let moved_any = self.md.maybe_moved(st, root as usize);
                if !moved_any && self.once(sp, "CR0381") {
                    diag::error(
                        "CR0381",
                        sp,
                        format!(
                            "cannot assign a field of `{}` before `{}` is initialized",
                            self.name(&rp),
                            self.name(&rp)
                        ),
                    )
                    .help(
                        "initialize the whole value at once, for example with a compound literal",
                    );
                }
            }
        }
    }
}

/// Reports initialization and move errors.
pub fn report(b: &body, prog: &program, md: &move_data, states: &[init_state]) {
    let mut r = reporter { b, prog, md, seen: HashSet::new(), move_sites: HashMap::new() };
    // Remember where each path is moved, for the "value moved here" note.
    for bb in &b.blocks {
        for s in &bb.stmts {
            for_each_place(&s.kind, |_, _| {});
            let mut record_move = |o: &Operand| {
                if let OperandMove(p) = o {
                    if let Some(path) = md.exact_path(p) {
                        r.move_sites.entry(path).or_insert(s.span);
                        for &l in &md.leaves[path] {
                            r.move_sites.entry(usize::MAX - l).or_insert(s.span);
                        }
                    }
                }
            };
            match &s.kind {
                StatementAssign(_, RvalueUse(o) | RvalueCast(_, o, _)) => record_move(o),
                StatementCall { args, .. } => args.iter().for_each(&mut record_move),
                _ => {}
            }
        }
    }
    let preds = b.predecessors();
    for (i, bb) in b.blocks.iter().enumerate() {
        if i != 0 && preds[i].is_empty() {
            continue;
        }
        let mut st = states[i].clone();
        for s in &bb.stmts {
            r.check_statement(&st, s);
            transfer(md, b, &mut st, s);
        }
        if let TermSwitch(o, ..) = &bb.term {
            if let Some(p) = o.place() {
                r.check_read(&st, p, bb.term_span, "use");
            }
        }
        if let TermReturn = bb.term {
            if !is_void(b.ret.ty)
                && md.maybe_uninit(&st, ReturnSlot as usize)
                && r.once(bb.term_span, "CR0381")
            {
                let name = prog.globals[b.func as usize].name;
                diag::error(
                    "CR0381",
                    bb.term_span,
                    format!("`{name}` can reach its end without returning a value"),
                )
                .help(format!("return a value of type `{}` on every path", display(b.ret)));
            }
        }
    }
}

/// The units that a drop of `path` releases: the path itself, or its owned
/// fields when the record has no drop function of its own.
fn drop_units(md: &move_data, path: usize, out: &mut Vec<usize>) {
    let ty = md.paths[path].ty.ty;
    if !is_owned(ty) {
        return;
    }
    if let TypeKindRecord(r) = kind(ty) {
        if record(r).drop.is_none() && !md.paths[path].children.is_empty() {
            for &c in &md.paths[path].children {
                drop_units(md, c, out);
            }
            return;
        }
    }
    out.push(path);
}

fn is_ancestor(md: &move_data, a: usize, mut b: usize) -> bool {
    loop {
        if a == b {
            return true;
        }
        match md.paths[b].parent {
            Some(p) => b = p,
            None => return false,
        }
    }
}

/// Replaces each `DropIfInit` with a static drop, a flagged drop, or nothing,
/// and maintains the drop flags.
pub fn elaborate_drops(b: &mut body, prog: &program, md: &move_data, states: &[init_state]) {
    let mut flags: HashMap<usize, local> = HashMap::new();
    let mut new_blocks = b.blocks.clone();
    for (i, bb) in b.blocks.iter().enumerate() {
        let mut st = states[i].clone();
        let mut out = Vec::new();
        for s in &bb.stmts {
            if let StatementDropIfInit(p) = &s.kind {
                if let Some(path) = md.exact_path(p) {
                    let mut units = Vec::new();
                    drop_units(md, path, &mut units);
                    for u in units {
                        let up = md.paths[u].place.clone();
                        let all = !md.maybe_uninit(&st, u);
                        let any = md.maybe_init(&st, u);
                        if all && any {
                            out.push(statement {
                                kind: StatementDrop(up),
                                span: s.span,
                                unchecked: s.unchecked,
                            });
                        } else if any {
                            let f = *flags.entry(u).or_insert_with(|| {
                                b.locals.push(local_decl {
                                    ty: q(bool_()),
                                    name: None,
                                    span: s.span,
                                    mutable: true,
                                    is_let: false,
                                    user: false,
                                    is_param: false,
                                    zero_init: false,
                                });
                                b.locals.len() as local - 1
                            });
                            out.push(statement {
                                kind: StatementDropIfFlag(up, f),
                                span: s.span,
                                unchecked: s.unchecked,
                            });
                        }
                    }
                }
                transfer(md, b, &mut st, s);
                continue;
            }
            transfer(md, b, &mut st, s);
            out.push(s.clone());
        }
        new_blocks[i].stmts = out;
    }
    b.blocks = new_blocks;
    if flags.is_empty() {
        return;
    }
    // Keep each flag equal to "this unit is initialized".
    for bb in b.blocks.iter_mut() {
        let mut out = Vec::new();
        for s in bb.stmts.drain(..) {
            let mut sets: Vec<(local, bool)> = Vec::new();
            let mut effect = |p: &place, init: bool| {
                if let Some(path) = md.exact_path(p) {
                    for (&u, &f) in &flags {
                        if is_ancestor(md, path, u) || (!init && is_ancestor(md, u, path)) {
                            sets.push((f, init));
                        }
                    }
                }
            };
            match &s.kind {
                StatementAssign(p, rv) => {
                    if let RvalueUse(OperandMove(x)) | RvalueCast(_, OperandMove(x), _) = rv {
                        effect(x, false);
                    }
                    effect(p, true);
                }
                StatementCall { dest, args, .. } => {
                    for a in args {
                        if let OperandMove(x) = a {
                            effect(x, false);
                        }
                    }
                    if let Some(d) = dest {
                        effect(d, true);
                    }
                }
                StatementDrop(p) => effect(p, false),
                _ => {}
            }
            let unchecked = s.unchecked;
            let sp = s.span;
            out.push(s);
            for (f, v) in sets {
                out.push(statement { kind: StatementSetFlag(f, v), span: sp, unchecked });
            }
        }
        bb.stmts = out;
    }
    let mut entry: Vec<statement> = flags
        .values()
        .map(|f| statement { kind: StatementSetFlag(*f, false), span: b.span, unchecked: true })
        .collect();
    entry.append(&mut b.blocks[0].stmts);
    b.blocks[0].stmts = entry;
    let _ = prog;
}

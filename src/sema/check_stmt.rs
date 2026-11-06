/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::tir::*;
use super::types::*;
use super::*;
use crate::diag;

impl sema<'_> {
    pub fn block(
        &mut self,
        items: &[BlockItem],
        sp: span,
        unsafe_: bool,
        new_scope: bool,
    ) -> tblock {
        if new_scope {
            self.push_scope();
        }
        self.fctx.as_mut().unwrap().blocks.push(Vec::new());
        if unsafe_ {
            self.fctx.as_mut().unwrap().unsafe_depth += 1;
        }
        let mut stmts = Vec::new();
        for it in items {
            self.block_item(it, &mut stmts);
        }
        if unsafe_ {
            self.fctx.as_mut().unwrap().unsafe_depth -= 1;
        }
        let locals = self.fctx.as_mut().unwrap().blocks.pop().unwrap();
        if new_scope {
            self.pop_scope();
        }
        tblock { stmts, locals, unsafe_, span: sp }
    }

    fn block_item(&mut self, it: &BlockItem, out: &mut Vec<tstmt>) {
        match it {
            BlockItemDecl(d) => out.extend(self.local_declaration(d)),
            BlockItemLet(l) => out.extend(self.local_let(l)),
            BlockItemStaticAssert(sa) => self.static_assert(sa),
            BlockItemStmt(s) => out.push(self.stmt(s)),
        }
    }

    fn cond(&mut self, e: &expr) -> texpr {
        let v = self.rvalue_expr(e);
        if !is_scalar(v.ty.ty) && !v.is_error() {
            diag::error(
                "CR1305",
                e.span,
                format!("condition has non-scalar type `{}`", display(v.ty)),
            );
            return texpr::error(e.span);
        }
        v
    }

    /// A statement that forms its own scope (C23 6.8.5).
    fn scoped(&mut self, s: &stmt) -> tstmt {
        if let StmtKindCompound(_) = s.kind {
            return self.stmt(s);
        }
        self.push_scope();
        self.fctx.as_mut().unwrap().blocks.push(Vec::new());
        let inner = self.stmt(s);
        let locals = self.fctx.as_mut().unwrap().blocks.pop().unwrap();
        self.pop_scope();
        if locals.is_empty() {
            return inner;
        }
        tstmt {
            span: s.span,
            kind: TStmtBlock(tblock { stmts: vec![inner], locals, unsafe_: false, span: s.span }),
        }
    }

    fn label_id(&mut self, name: sym, sp: span) -> label_id {
        let fc = self.fctx.as_mut().unwrap();
        if let Some(&id) = fc.label_map.get(&name) {
            return id;
        }
        fc.labels.push(label_info { name, span: sp, defined: false });
        let id = fc.labels.len() as label_id - 1;
        fc.label_map.insert(name, id);
        id
    }

    fn anon_label(&mut self, sp: span) -> label_id {
        let fc = self.fctx.as_mut().unwrap();
        fc.labels.push(label_info { name: SymEmpty, span: sp, defined: true });
        fc.labels.len() as label_id - 1
    }

    pub fn stmt(&mut self, s: &stmt) -> tstmt {
        let sp = s.span;
        let kind = match &s.kind {
            StmtKindCompound(items) => TStmtBlock(self.block(items, sp, false, true)),
            StmtKindUnsafe(items) => {
                if !self.crust() {
                    diag::ice("unsafe block in C mode");
                }
                TStmtBlock(self.block(items, sp, true, true))
            }
            StmtKindExpr(None) => TStmtNop,
            StmtKindExpr(Some(e)) => {
                let v = self.expr(e);
                let v = if v.lvalue && !is_array(v.ty.ty) && !is_function(v.ty.ty) {
                    self.to_rvalue(v)
                } else {
                    v
                };
                TStmtExpr(v)
            }
            StmtKindIf(c, t, e) => {
                let c = self.cond(c);
                let t = self.scoped(t);
                let e = e.as_ref().map(|e| Box::new(self.scoped(e)));
                TStmtIf(c, Box::new(t), e)
            }
            StmtKindWhile(c, body) => {
                let c = self.cond(c);
                let body = self.loop_body(body);
                TStmtWhile(c, Box::new(body))
            }
            StmtKindDoWhile(body, c) => {
                let body = self.loop_body(body);
                let c = self.cond(c);
                TStmtDoWhile(Box::new(body), c)
            }
            StmtKindFor(init, c, step, body) => return self.for_stmt(init, c, step, body, sp),
            StmtKindSwitch(c, body) => self.switch(c, body, sp),
            StmtKindLabeled(lk, lsp, body) => match lk {
                LabelKindNamed(name) => {
                    let id = self.label_id(*name, *lsp);
                    let fc = self.fctx.as_mut().unwrap();
                    if fc.labels[id as usize].defined {
                        diag::error("CR1312", *lsp, format!("redefinition of label `{name}`"));
                    }
                    fc.labels[id as usize].defined = true;
                    fc.labels[id as usize].span = *lsp;
                    let b = self.stmt(body);
                    TStmtLabel(id, Box::new(b))
                }
                LabelKindCase(e) => {
                    let id = self.anon_label(*lsp);
                    let v = self.const_int_expr(e);
                    match self.fctx.as_mut().unwrap().switches.last_mut() {
                        None => {
                            diag::error("CR1312", *lsp, "`case` label outside a `switch`");
                        }
                        Some(sw) => {
                            if let Some(v) = v {
                                let v = super::consteval::wrap_to(v, promote(sw.ty));
                                if let Some((_, _, prev)) = sw.cases.iter().find(|c| c.0 == v) {
                                    diag::error(
                                        "CR1313",
                                        *lsp,
                                        format!("duplicate case value `{v}`"),
                                    )
                                    .secondary(*prev, "first used here");
                                } else {
                                    sw.cases.push((v, id, *lsp));
                                }
                            }
                        }
                    }
                    let b = self.stmt(body);
                    TStmtLabel(id, Box::new(b))
                }
                LabelKindDefault => {
                    let id = self.anon_label(*lsp);
                    match self.fctx.as_mut().unwrap().switches.last_mut() {
                        None => {
                            diag::error("CR1312", *lsp, "`default` label outside a `switch`");
                        }
                        Some(sw) => {
                            if sw.default.is_some() {
                                diag::error(
                                    "CR1313",
                                    *lsp,
                                    "multiple `default` labels in one `switch`",
                                );
                            }
                            sw.default = Some(id);
                        }
                    }
                    let b = self.stmt(body);
                    TStmtLabel(id, Box::new(b))
                }
            },
            StmtKindBreak => {
                if self.fctx.as_ref().unwrap().breakable == 0 {
                    diag::error("CR1312", sp, "`break` outside a loop or `switch`");
                }
                TStmtBreak
            }
            StmtKindContinue => {
                if self.fctx.as_ref().unwrap().loops == 0 {
                    diag::error("CR1312", sp, "`continue` outside a loop");
                }
                TStmtContinue
            }
            StmtKindGoto(name) => {
                let id = self.label_id(*name, sp);
                TStmtGoto(id)
            }
            StmtKindReturn(e) => self.return_stmt(e.as_ref(), sp),
        };
        tstmt { kind, span: sp }
    }

    fn loop_body(&mut self, body: &stmt) -> tstmt {
        let fc = self.fctx.as_mut().unwrap();
        fc.loops += 1;
        fc.breakable += 1;
        let b = self.scoped(body);
        let fc = self.fctx.as_mut().unwrap();
        fc.loops -= 1;
        fc.breakable -= 1;
        b
    }

    fn for_stmt(
        &mut self,
        init: &Option<Box<BlockItem>>,
        c: &Option<expr>,
        step: &Option<expr>,
        body: &stmt,
        sp: span,
    ) -> tstmt {
        self.push_scope();
        self.fctx.as_mut().unwrap().blocks.push(Vec::new());
        let mut stmts = Vec::new();
        if let Some(i) = init {
            self.block_item(i, &mut stmts);
        }
        let c = c.as_ref().map(|c| self.cond(c));
        let step = step.as_ref().map(|s| {
            let v = self.expr(s);
            if v.lvalue && !is_array(v.ty.ty) { self.to_rvalue(v) } else { v }
        });
        let b = self.loop_body(body);
        stmts.push(tstmt { kind: TStmtFor(c, step, Box::new(b)), span: sp });
        let locals = self.fctx.as_mut().unwrap().blocks.pop().unwrap();
        self.pop_scope();
        tstmt { kind: TStmtBlock(tblock { stmts, locals, unsafe_: false, span: sp }), span: sp }
    }

    fn switch(&mut self, c: &expr, body: &stmt, sp: span) -> TStmt {
        let v = self.rvalue_expr(c);
        if !is_integer(v.ty.ty) && !v.is_error() {
            diag::error(
                "CR1313",
                c.span,
                format!("`switch` needs an integer, found `{}`", display(v.ty)),
            );
        }
        let orig_ty = match &v.kind {
            TExprLoad(x) => x.ty.ty,
            _ => v.ty.ty,
        };
        let pt = promote(v.ty.ty);
        let v = if pt != v.ty.ty { self.cast_to(v, q(pt)) } else { v };
        let fc = self.fctx.as_mut().unwrap();
        fc.switches.push(switch_ctx { cases: Vec::new(), default: None, ty: orig_ty });
        fc.breakable += 1;
        let b = self.scoped(body);
        let fc = self.fctx.as_mut().unwrap();
        fc.breakable -= 1;
        let sw = fc.switches.pop().unwrap();
        if let TypeKindEnum(e) = kind(orig_ty) {
            if sw.default.is_none() {
                let ed = enum_info(e);
                let missing: Vec<String> = ed
                    .enumerators
                    .iter()
                    .filter(|(_, val)| !sw.cases.iter().any(|c| c.0 == *val))
                    .map(|(n, _)| format!("`{n}`"))
                    .collect();
                if !missing.is_empty() {
                    diag::error(
                        "CR0004",
                        c.span,
                        format!(
                            "`switch` over `{}` does not handle {}",
                            display(q(orig_ty)),
                            missing.join(", ")
                        ),
                    )
                    .help("add a `case` for each missing enumerator, or a `default` label");
                }
            }
        }
        let cases = sw.cases.iter().map(|c| (c.0, c.1)).collect();
        let _ = sp;
        TStmtSwitch(v, Box::new(b), cases, sw.default)
    }

    fn return_stmt(&mut self, e: Option<&expr>, sp: span) -> TStmt {
        let ret = self.fctx.as_ref().unwrap().ret;
        match e {
            None => {
                if !is_void(ret.ty) && !is_error(ret.ty) {
                    diag::error(
                        "CR1317",
                        sp,
                        format!("non-void function must return a value of type `{}`", display(ret)),
                    );
                }
                TStmtReturn(None)
            }
            Some(e) => {
                let v = self.expr(e);
                if is_void(ret.ty) {
                    let v = self.to_rvalue(v);
                    if !is_void(v.ty.ty) && !v.is_error() {
                        diag::error("CR1317", e.span, "a `void` function cannot return a value");
                    }
                    return TStmtExpr(v);
                }
                let v = self.convert_assign(v, ret.unqual(), e.span, "return value");
                TStmtReturn(Some(v))
            }
        }
    }
}

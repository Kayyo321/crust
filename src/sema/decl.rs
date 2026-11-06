/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use super::tir::*;
use super::types::*;
use super::*;
use crate::diag;

impl sema<'_> {
    fn namespace_tag(&self, ns: sym, tag: sym, sp: span) -> Option<Tag> {
        if let Some(iface) = self.namespaces.get(&ns) {
            for e in iface.find(tag) {
                match e.kind {
                    ExportKindRecord(r) => return Some(TagRecord(r)),
                    ExportKindEnum(en) => return Some(TagEnum(en)),
                    _ => {}
                }
            }
        }
        diag::error(
            "CR1302",
            sp,
            format!("`{ns}::{tag}` is not an exported struct, union, or enum"),
        );
        None
    }

    pub fn record_spec(&mut self, r: &record_spec, attrs: &[attribute]) -> type_id {
        if let (Some(ns), Some((tag, sp))) = (r.ns, r.tag) {
            return match self.namespace_tag(ns, tag, sp) {
                Some(TagRecord(id)) => intern(TypeKindRecord(id)),
                _ => error(),
            };
        }
        let kw = if r.is_union { "union" } else { "struct" };
        let id = match (&r.tag, &r.fields) {
            (Some((tag, sp)), None) => {
                if let Some(t) = self.lookup_tag(*tag) {
                    match t {
                        TagRecord(id) if record(id).is_union == r.is_union => {
                            return intern(TypeKindRecord(id));
                        }
                        _ => {
                            diag::error(
                                "CR1303",
                                *sp,
                                format!("`{tag}` is declared as a different kind of tag"),
                            );
                            return error();
                        }
                    }
                }
                let id = self.new_incomplete_record(r.is_union, Some(*tag), *sp);
                self.scopes.last_mut().unwrap().tags.insert(*tag, TagRecord(id));
                return intern(TypeKindRecord(id));
            }
            (tag, Some(_)) => {
                let existing =
                    tag.and_then(|(t, _)| self.scopes.last().unwrap().tags.get(&t).copied());
                match existing {
                    Some(TagRecord(id))
                        if !record(id).complete && record(id).is_union == r.is_union =>
                    {
                        id
                    }
                    Some(_) => {
                        let (t, sp) = tag.unwrap();
                        diag::error("CR1303", sp, format!("redefinition of `{kw} {t}`"));
                        self.new_incomplete_record(r.is_union, Some(t), sp)
                    }
                    None => {
                        let id = self.new_incomplete_record(r.is_union, tag.map(|t| t.0), r.span);
                        if let Some((t, _)) = tag {
                            self.scopes.last_mut().unwrap().tags.insert(*t, TagRecord(id));
                        }
                        id
                    }
                }
            }
            (None, None) => return error(),
        };
        if let Some((t, _)) = r.tag {
            if self.at_file_scope() {
                self.file_tags.push((t, TagRecord(id)));
            }
        }
        self.define_record(id, r, attrs);
        intern(TypeKindRecord(id))
    }

    /// The module that owns a declaration: only the module file itself, not
    /// the C headers it includes.
    pub fn owner_module(&self, sp: span) -> Option<sym> {
        self.module.filter(|_| sp.file == self.main_file)
    }

    fn new_incomplete_record(&mut self, is_union: bool, tag: Option<sym>, sp: span) -> u32 {
        let module = self.owner_module(sp);
        if self.at_file_scope() {
            if let Some(t) = tag {
                if let Some(id) = crate::modules::interface::lookup_record(module, t) {
                    if module.is_none() && record(id).is_union == is_union {
                        return id;
                    }
                }
            }
        }
        let id = self.new_record_raw(is_union, tag, sp, module);
        if let (Some(t), true) = (tag, self.at_file_scope()) {
            crate::modules::interface::register_record(module, t, id);
        }
        id
    }

    fn new_record_raw(
        &mut self,
        is_union: bool,
        tag: Option<sym>,
        sp: span,
        module: Option<sym>,
    ) -> u32 {
        new_record(record_def {
            is_union,
            tag,
            module,
            complete: false,
            fields: Vec::new(),
            size: 0,
            align: 1,
            drop: None,
            owned: false,
            span: sp,
        })
    }

    fn define_record(&mut self, id: u32, r: &record_spec, outer_attrs: &[attribute]) {
        let mut fields = Vec::new();
        let mut widths = Vec::new();
        let decls = r.fields.as_ref().unwrap();
        let n = decls.len();
        for (fi, fd) in decls.iter().enumerate() {
            if let Some(sa) = &fd.static_assert {
                self.static_assert(sa);
                continue;
            }
            let base = self.base_type(&fd.spec);
            if fd.declarators.is_empty() {
                // An anonymous struct or union member.
                if is_record(base.ty) {
                    fields.push(field {
                        name: None,
                        ty: base,
                        offset: 0,
                        bits: None,
                        span: fd.span,
                    });
                    widths.push(None);
                } else {
                    diag::error("CR1315", fd.span, "declaration does not declare a member");
                }
                continue;
            }
            for (k, (d, w)) in fd.declarators.iter().enumerate() {
                let (ty, name, sp) = match d {
                    Some(d) => {
                        let (t, _) = self.apply_declarator(base, d, false);
                        (t, d.name.map(|n| n.0), d.name.map(|n| n.1).unwrap_or(d.span))
                    }
                    None => (base, None, fd.span),
                };
                let width = match w {
                    Some(we) => {
                        if !is_integer(ty.ty) {
                            diag::error("CR1315", sp, "a bit-field must have an integer type");
                        }
                        let v = self.const_int_expr(we).unwrap_or(1);
                        let max = size_of(ty.ty) * 8;
                        if v < 0 || v as u64 > max || (v == 0 && name.is_some()) {
                            diag::error("CR1308", we.span, format!("invalid bit-field width {v}"));
                        }
                        Some(v.clamp(0, max as i128) as u32)
                    }
                    None => None,
                };
                let last = fi == n - 1 && k == fd.declarators.len() - 1;
                if let TypeKindArray(_, None) = ty.kind() {
                    if !last || r.is_union {
                        diag::error(
                            "CR1310",
                            sp,
                            "a flexible array member must be the last member of a struct",
                        );
                    }
                } else if !is_complete(ty.ty) && !is_error(ty.ty) {
                    diag::error(
                        "CR1310",
                        sp,
                        format!("member has incomplete type `{}`", display(ty)),
                    );
                }
                if is_function(ty.ty) {
                    diag::error("CR1315", sp, "a member cannot have function type");
                }
                if is_ref(ty.ty) || is_slice(ty.ty) {
                    diag::error("CR1321", sp, "a struct or union member cannot hold a borrow in Crust 0.1")
                        .help("store a raw pointer and borrow at the use site, or pass the borrow as a parameter");
                }
                if r.is_union && is_owned(ty.ty) {
                    diag::error(
                        "CR0702",
                        sp,
                        format!("a union member cannot have the owned type `{}`", display(ty)),
                    )
                    .note(
                        "the checker cannot know which member is active, so it could not drop it",
                    );
                }
                if let Some(nm) = name {
                    if fields.iter().any(|f: &field| f.name == Some(nm)) {
                        diag::error("CR1303", sp, format!("duplicate member `{nm}`"));
                    }
                }
                if width.is_some() && name.is_none() && width != Some(0) {
                    // Unnamed bit-fields are padding.
                }
                fields.push(field { name, ty, offset: 0, bits: None, span: sp });
                widths.push(width);
            }
        }
        let attrs: Vec<&attribute> = r.attrs.iter().chain(outer_attrs.iter()).collect();
        let mut min_align = 1;
        for a in &attrs {
            if a.prefix.is_none() && a.name.as_str().trim_matches('_') == "packed" {
                diag::warning("CR1320", a.span, "`packed` is ignored by Crust 0.1");
            }
            if a.name.as_str().trim_matches('_') == "aligned" {
                min_align = 16;
            }
        }
        let (size, align) = layout_record(r.is_union, &mut fields, &widths, min_align);
        let mut drop_fn = None;
        for a in &attrs {
            if a.is_crust(SymDrop) {
                let name = a.args.as_ref().and_then(|args| {
                    if args.len() == 1 && args[0].kind == crate::lex::TokenKindIdent {
                        Some(args[0].text)
                    } else {
                        None
                    }
                });
                match name {
                    Some(n) => drop_fn = Some((n, a.span)),
                    None => {
                        diag::error(
                            "CR1320",
                            a.span,
                            "`crust::drop` takes the name of a drop function: `[[crust::drop(f)]]`",
                        );
                    }
                }
            }
        }
        let owned = drop_fn.is_some() || fields.iter().any(|f| is_owned(f.ty.ty));
        if drop_fn.is_some() && r.is_union {
            diag::error("CR1320", r.span, "`crust::drop` applies to structs, not unions");
        }
        update_record(id, |rd| {
            rd.fields = fields;
            rd.size = size;
            rd.align = align;
            rd.complete = true;
            rd.owned = owned;
            rd.span = r.span;
        });
        if let Some((f, sp)) = drop_fn {
            self.pending_drops.push(pending_drop { record: id, func: f, span: sp });
        }
    }

    pub fn enum_spec(&mut self, e: &enum_spec) -> type_id {
        if let (Some(ns), Some((tag, sp))) = (e.ns, e.tag) {
            return match self.namespace_tag(ns, tag, sp) {
                Some(TagEnum(id)) => intern(TypeKindEnum(id)),
                _ => error(),
            };
        }
        let fixed = e.fixed.as_ref().map(|tn| {
            let t = self.type_name(tn);
            match int_kind(t.ty) {
                Some(k) if !matches!(kind(t.ty), TypeKindEnum(_)) => k,
                _ => {
                    diag::error(
                        "CR1315",
                        tn.span,
                        "an enum's underlying type must be an integer type",
                    );
                    IntKindInt
                }
            }
        });
        if e.enumerators.is_none() {
            if let Some((tag, sp)) = e.tag {
                if let Some(t) = self.lookup_tag(tag) {
                    return match t {
                        TagEnum(id) => intern(TypeKindEnum(id)),
                        _ => {
                            diag::error(
                                "CR1303",
                                sp,
                                format!("`{tag}` is declared as a different kind of tag"),
                            );
                            error()
                        }
                    };
                }
                let id = new_enum(enum_def {
                    tag: Some(tag),
                    module: self.owner_module(sp),
                    complete: false,
                    underlying: fixed.unwrap_or(IntKindUInt),
                    fixed: fixed.is_some(),
                    enumerators: vec![],
                    span: sp,
                });
                self.scopes.last_mut().unwrap().tags.insert(tag, TagEnum(id));
                return intern(TypeKindEnum(id));
            }
            return error();
        }
        let existing = e.tag.and_then(|(t, _)| self.scopes.last().unwrap().tags.get(&t).copied());
        let id = match existing {
            Some(TagEnum(id)) if !enum_info(id).complete => id,
            Some(_) => {
                diag::error("CR1303", e.span, "redefinition of enum");
                return error();
            }
            None => {
                let id = new_enum(enum_def {
                    tag: e.tag.map(|t| t.0),
                    module: self.owner_module(e.span),
                    complete: false,
                    underlying: fixed.unwrap_or(IntKindUInt),
                    fixed: fixed.is_some(),
                    enumerators: vec![],
                    span: e.span,
                });
                if let Some((t, _)) = e.tag {
                    self.scopes.last_mut().unwrap().tags.insert(t, TagEnum(id));
                    if self.at_file_scope() {
                        crate::modules::interface::register_enum(self.owner_module(e.span), t, id);
                    }
                }
                id
            }
        };
        if let Some((t, _)) = e.tag {
            if self.at_file_scope() {
                self.file_tags.push((t, TagEnum(id)));
            }
        }
        let ety = intern(TypeKindEnum(id));
        let mut next: i128 = 0;
        let mut values = Vec::new();
        for en in e.enumerators.as_ref().unwrap() {
            let v = match &en.value {
                Some(x) => self.const_int_expr(x).unwrap_or(next),
                None => next,
            };
            if let Some(k) = fixed {
                if crate::sema::consteval::wrap_to(v, int_of(k)) != v {
                    diag::error(
                        "CR1308",
                        en.span,
                        format!(
                            "enumerator value {v} does not fit the underlying type `{}`",
                            int_name(k)
                        ),
                    );
                }
            }
            values.push((en.name, v));
            let cty = if fixed.is_some() {
                ety
            } else if fits(v, IntKindInt) {
                int()
            } else {
                int_of(IntKindLong)
            };
            self.scopes
                .last_mut()
                .unwrap()
                .ord
                .insert(en.name, BindingConst(ConstValueInt(v), q(cty)));
            next = v + 1;
        }
        let underlying = fixed.unwrap_or_else(|| {
            let min = values.iter().map(|v| v.1).min().unwrap_or(0);
            let max = values.iter().map(|v| v.1).max().unwrap_or(0);
            if min >= 0 && max <= u32::MAX as i128 {
                IntKindUInt
            } else if fits(min, IntKindInt) && fits(max, IntKindInt) {
                IntKindInt
            } else if min >= 0 && max > i64::MAX as i128 {
                IntKindULong
            } else {
                IntKindLong
            }
        });
        update_enum(id, |ed| {
            ed.complete = true;
            ed.underlying = underlying;
            ed.enumerators = values.clone();
        });
        if fixed.is_none() {
            // C23: if every value fits `int`, the constants have type `int`;
            // otherwise they have the enumerated type.
            let all_int = values.iter().all(|v| fits(v.1, IntKindInt));
            if !all_int {
                for (n, v) in &values {
                    self.scopes
                        .last_mut()
                        .unwrap()
                        .ord
                        .insert(*n, BindingConst(ConstValueInt(*v), q(ety)));
                }
            }
        }
        ety
    }

    // ---- block-scope declarations ----

    pub fn local_declaration(&mut self, d: &declaration) -> Vec<tstmt> {
        self.validate_attrs_local(&d.spec.attrs);
        let mut out = Vec::new();
        if d.inits.is_empty() {
            let _ = self.base_type(&d.spec);
            return out;
        }
        let auto = matches!(d.spec.ty, TypeSpecAuto);
        let base = if auto { q(error()) } else { self.base_type(&d.spec) };
        for id in &d.inits {
            if let Some(s) = self.local_declarator(&d.spec, base, id, auto) {
                out.push(s);
            }
        }
        out
    }

    fn validate_attrs_local(&self, attrs: &[attribute]) {
        for a in attrs {
            if a.prefix == Some(SymCrust) && !matches!(a.name, SymDrop | SymSafe | SymCAbi) {
                diag::error("CR1320", a.span, format!("unknown attribute `crust::{}`", a.name));
            }
        }
    }

    fn local_declarator(
        &mut self,
        spec: &decl_spec,
        base: qual_type,
        id: &init_declarator,
        auto: bool,
    ) -> Option<tstmt> {
        let d = &id.declarator;
        let (name, nsp) = d.name?;
        let storage = spec.storage.map(|s| s.0);
        if auto {
            let Some(InitializerExpr(e)) = &id.init else {
                diag::error(
                    "CR1322",
                    nsp,
                    "`auto` needs a plain initializer expression to infer the type",
                );
                return None;
            };
            let te = self.expr(e);
            let te = self.to_rvalue(te);
            let ty = te.ty.unqual().with(spec.quals);
            let lid = self.new_local(name, ty, nsp, true, false);
            self.scopes.last_mut().unwrap().ord.insert(name, BindingLocal(lid));
            let init = self.convert_assign(te, ty, e.span, "initializer");
            return Some(tstmt { kind: TStmtDecl(lid, Some(TInitExpr(init))), span: nsp });
        }
        let (ty, _) = self.apply_declarator(base, d, false);
        if storage == Some(StorageTypedef) {
            self.scopes.last_mut().unwrap().ord.insert(name, BindingTypedef(ty));
            return None;
        }
        if is_function(ty.ty) || storage == Some(StorageExtern) {
            let g = self.declare_global(name, ty, is_function(ty.ty), nsp, false);
            if spec.unsafe_.is_some() {
                self.prog.globals[g as usize].unsafe_fn = true;
            }
            if spec.attrs.iter().any(|a| a.is_crust(SymSafe)) {
                self.prog.globals[g as usize].safe_attr = true;
            }
            self.scopes.last_mut().unwrap().ord.insert(name, BindingGlobal(g));
            return None;
        }
        if storage == Some(StorageStatic) || spec.thread_local {
            let fc = self.fctx.as_mut().unwrap();
            fc.static_count += 1;
            let sym_name = format!("{}.{}.{}", fc.name, name, fc.static_count);
            let mut gd = self.new_global(name, ty, false, nsp);
            gd.symbol = sym_name;
            gd.linkage = LinkageInternal;
            gd.defined = true;
            gd.thread_local = spec.thread_local;
            self.prog.globals.push(gd);
            let g = self.prog.globals.len() as global_id - 1;
            self.scopes.last_mut().unwrap().ord.insert(name, BindingGlobal(g));
            match &id.init {
                Some(init) => self.global_init(g, init, spec.constexpr_),
                None => self.prog.globals[g as usize].tentative = true,
            }
            return None;
        }
        let lid = self.new_local(name, ty, nsp, true, false);
        self.scopes.last_mut().unwrap().ord.insert(name, BindingLocal(lid));
        let init = match &id.init {
            Some(i) => {
                let (ti, fixed_ty) = self.local_init(ty, i);
                if fixed_ty != ty {
                    self.fctx.as_mut().unwrap().locals[lid as usize].ty = fixed_ty;
                }
                Some(ti)
            }
            None => {
                if spec.constexpr_ {
                    diag::error("CR1308", nsp, "a `constexpr` object needs an initializer");
                }
                if !is_complete(ty.ty) && !is_error(ty.ty) {
                    diag::error(
                        "CR1310",
                        nsp,
                        format!("variable `{name}` has incomplete type `{}`", display(ty)),
                    );
                }
                None
            }
        };
        if spec.constexpr_ {
            if let Some(TInitExpr(e)) = &init {
                if let Some(v) = crate::sema::consteval::eval(e) {
                    if is_scalar(ty.ty) {
                        self.fctx.as_mut().unwrap().constexpr_locals.insert(lid, v);
                    }
                } else {
                    diag::error(
                        "CR1308",
                        nsp,
                        "a `constexpr` initializer must be a constant expression",
                    );
                }
            }
            let fc = self.fctx.as_mut().unwrap();
            fc.locals[lid as usize].ty = fc.locals[lid as usize].ty.with(QualConst);
        }
        Some(tstmt { kind: TStmtDecl(lid, init), span: nsp })
    }

    pub fn local_let(&mut self, l: &let_decl) -> Option<tstmt> {
        let declared = l.ty.as_ref().map(|t| self.type_name(t));
        let (ty, init) = match (&l.init, declared) {
            (Some(i), Some(t)) => {
                let (ti, ft) = self.local_init(t, i);
                (ft, Some(ti))
            }
            (Some(InitializerExpr(e)), None) => {
                let te = self.expr(e);
                let te = self.to_rvalue(te);
                if is_void(te.ty.ty) {
                    diag::error("CR1319", e.span, "cannot bind a `void` value");
                }
                let t = te.ty.unqual();
                (t, Some(TInitExpr(te)))
            }
            (Some(InitializerList(_, sp)), None) => {
                diag::error(
                    "CR1322",
                    *sp,
                    "a braced initializer needs a type: write `let name: T = { ... };`",
                );
                (q(error()), None)
            }
            (None, Some(t)) => (t, None),
            (None, None) => {
                // Inferred from the first assignment (docs/LANGUAGE.md, section 3).
                (q(error()), None)
            }
        };
        let deferred = l.init.is_none() && l.ty.is_none();
        let ty = if l.mutable || deferred { ty } else { ty.with(QualConst) };
        if !is_complete(ty.ty) && !is_error(ty.ty) {
            diag::error(
                "CR1310",
                l.name_span,
                format!("`{}` has incomplete type `{}`", l.name, display(ty)),
            );
        }
        let lid = self.new_local(l.name, ty, l.name_span, l.mutable, true);
        if deferred {
            self.deferred_lets.insert(lid);
        }
        self.scopes.last_mut().unwrap().ord.insert(l.name, BindingLocal(lid));
        Some(tstmt { kind: TStmtDecl(lid, init), span: l.span })
    }
}

fn fits(v: i128, k: IntKind) -> bool {
    crate::sema::consteval::wrap_to(v, int_of(k)) == v
}

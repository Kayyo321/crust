/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/

pub mod check_expr;
pub mod check_stmt;
pub mod consteval;
pub mod decl;
pub mod initializer;
pub mod tir;
pub mod types;

use crate::diag::{self, FileKind, FileKindCr, FileKindCrm, FileKindHr, span};
use crate::lex::*;
use crate::modules::*;
use crate::parse::ast::*;
use std::collections::HashMap;
use std::rc::Rc;
use tir::*;
use types::*;

#[derive(Clone, Debug)]
pub enum Binding {
    BindingLocal(local_id),
    BindingGlobal(global_id),
    BindingTypedef(qual_type),
    BindingConst(ConstValue, qual_type),
}
pub use Binding::*;

#[derive(Clone, Copy, Debug)]
pub enum Tag {
    TagRecord(u32),
    TagEnum(u32),
}
pub use Tag::*;

#[derive(Default)]
pub struct scope {
    pub ord: HashMap<sym, Binding>,
    pub tags: HashMap<sym, Tag>,
}

pub struct switch_ctx {
    pub cases: Vec<(i128, label_id, span)>,
    pub default: Option<label_id>,
    pub ty: type_id,
}

pub struct func_ctx {
    pub locals: Vec<local_info>,
    pub ret: qual_type,
    pub labels: Vec<label_info>,
    pub label_map: HashMap<sym, label_id>,
    pub loops: u32,
    pub breakable: u32,
    pub switches: Vec<switch_ctx>,
    pub unsafe_depth: u32,
    pub unsafe_fn: bool,
    pub crust: bool,
    pub blocks: Vec<Vec<local_id>>,
    pub name: sym,
    pub static_count: u32,
}

/// A `[[crust::drop(f)]]` attribute waiting for `f` to be declared.
pub(crate) struct pending_drop {
    pub record: u32,
    pub func: sym,
    pub span: span,
}

pub struct sema<'a> {
    pub prog: program,
    pub scopes: Vec<scope>,
    pub fctx: Option<func_ctx>,
    pub env: &'a dyn module_env,
    pub module: Option<sym>,
    pub main_file: u32,
    pub kind: FileKind,
    pub namespaces: HashMap<sym, Rc<module_interface>>,
    /// Names imported into file scope, for duplicate checks.
    pub imported: HashMap<sym, span>,
    /// Names exported with `export` on a declaration or in an export list.
    export_names: Vec<(sym, span)>,
    export_renames: Vec<(sym, sym)>,
    reexports: Vec<export_item>,
    pub(crate) pending_drops: Vec<pending_drop>,
    /// Records and enums defined at file scope by this unit, by tag.
    pub file_tags: Vec<(sym, Tag)>,
    va_list: Option<type_id>,
    string_map: HashMap<(Vec<u32>, u8), u32>,
    /// `let x;` bindings whose type comes from their first assignment.
    pub deferred_lets: std::collections::HashSet<local_id>,
    /// Nesting depth of `sizeof`, `typeof`, and similar unevaluated operands.
    pub unevaluated: u32,
}

thread_local! {
    static Unchecked: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `--unchecked`: turns off `unsafe` gating for this compilation.
pub fn set_unchecked(v: bool) {
    Unchecked.with(|c| c.set(v));
}

pub struct sema_output {
    pub prog: program,
    pub exports: Vec<export_item>,
    pub module: Option<sym>,
}

pub fn analyze(tu: &translation_unit, env: &dyn module_env) -> sema_output {
    let mut s = sema {
        prog: program::default(),
        scopes: vec![scope::default()],
        fctx: None,
        env,
        module: tu.module.map(|m| m.0),
        main_file: tu.file,
        kind: tu.kind,
        namespaces: HashMap::new(),
        imported: HashMap::new(),
        export_names: Vec::new(),
        export_renames: Vec::new(),
        reexports: Vec::new(),
        pending_drops: Vec::new(),
        file_tags: Vec::new(),
        va_list: None,
        string_map: HashMap::new(),
        unevaluated: 0,
        deferred_lets: Default::default(),
    };
    let va = s.va_list_type();
    s.scopes[0].ord.insert(SymBuiltinVaList, BindingTypedef(q(va)));
    s.scopes[0].ord.insert(SymInt128T, BindingTypedef(q(int_of(IntKindInt128))));
    s.scopes[0].ord.insert(SymUInt128T, BindingTypedef(q(int_of(IntKindUInt128))));
    for it in &tu.items {
        s.item(it);
    }
    s.finalize();
    let exports = s.collect_exports();
    sema_output { prog: s.prog, exports, module: s.module }
}

impl<'a> sema<'a> {
    // ---- context helpers ----

    pub fn is_crust_span(&self, sp: span) -> bool {
        !sp.is_dummy() && diag::file_kind(sp.file).is_crust()
    }

    /// Whether the code being checked is Crust code (not C mode).
    pub fn crust(&self) -> bool {
        match &self.fctx {
            Some(f) => f.crust,
            None => self.kind.is_crust(),
        }
    }

    pub fn in_unsafe(&self) -> bool {
        match &self.fctx {
            Some(f) => f.unsafe_depth > 0 || f.unsafe_fn,
            None => false,
        }
    }

    /// Reports CR0133 when an unsafe operation appears in checked Crust code
    /// outside `unsafe`. C-mode files have no `unsafe` gating (D-037).
    pub fn require_unsafe(&self, sp: span, what: &str) {
        if self.unevaluated > 0
            || self.fctx.is_none()
            || !self.crust()
            || self.in_unsafe()
            || Unchecked.with(|c| c.get())
        {
            return;
        }
        diag::error("CR0133", sp, format!("{what} is unsafe and requires an `unsafe` block"))
            .label(format!("{what}"))
            .help(
                "wrap it in `unsafe { ... }` with a `// SAFETY:` comment that says why it is sound",
            );
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(scope::default());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn lookup(&self, name: sym) -> Option<Binding> {
        for s in self.scopes.iter().rev() {
            if let Some(o) = s.ord.get(&name) {
                return Some(o.clone());
            }
        }
        None
    }

    pub fn lookup_tag(&self, name: sym) -> Option<Tag> {
        for s in self.scopes.iter().rev() {
            if let Some(t) = s.tags.get(&name) {
                return Some(*t);
            }
        }
        None
    }

    pub fn at_file_scope(&self) -> bool {
        self.scopes.len() == 1
    }

    pub fn intern_string(&mut self, units: Vec<u32>, enc: crate::parse::literal::Encoding) -> u32 {
        let key = (units.clone(), enc as u8);
        if let Some(&id) = self.string_map.get(&key) {
            return id;
        }
        let elem = match enc {
            crate::parse::literal::EncodingPlain => char_(),
            crate::parse::literal::EncodingUtf8 => int_of(IntKindUChar),
            crate::parse::literal::EncodingUtf16 => int_of(IntKindUShort),
            crate::parse::literal::EncodingUtf32 => int_of(IntKindUInt),
            crate::parse::literal::EncodingWide => int(),
        };
        let ty = array_of(q(elem), Some(units.len() as u64 + 1));
        let id = self.prog.strings.len() as u32;
        self.prog.strings.push(string_lit { units, encoding: enc, ty });
        self.string_map.insert(key, id);
        id
    }

    /// `__builtin_va_list` on x86-64: `struct __va_list_tag[1]`.
    pub fn va_list_type(&mut self) -> type_id {
        if let Some(t) = self.va_list {
            return t;
        }
        let u32t = q(int_of(IntKindUInt));
        let vp = q(pointer_to(q(void())));
        let mut fields: Vec<field> = [u32t, u32t, vp, vp]
            .iter()
            .enumerate()
            .map(|(i, t)| field {
                name: Some(sym::intern(&format!("f{i}"))),
                ty: *t,
                offset: 0,
                bits: None,
                span: span::dummy(),
            })
            .collect();
        let (size, align) = layout_record(false, &mut fields, &[None, None, None, None], 1);
        let r = new_record(record_def {
            is_union: false,
            tag: Some(sym::intern("__va_list_tag")),
            module: None,
            complete: true,
            fields,
            size,
            align,
            drop: None,
            owned: false,
            span: span::dummy(),
        });
        let t = array_of(q(intern(TypeKindRecord(r))), Some(1));
        self.va_list = Some(t);
        t
    }

    pub fn is_va_list(&mut self, t: type_id) -> bool {
        let v = self.va_list_type();
        t == v
    }

    // ---- items ----

    fn item(&mut self, it: &item) {
        match &it.kind {
            ItemKindImport(d) => self.import(d),
            ItemKindExportList(l) => self.export_list(l, it.exported),
            ItemKindDecl(d) => self.declaration(d, it.exported),
            ItemKindFunc(f) => self.function_def(f, it.exported),
            ItemKindLet(l) => self.file_let(l, it.exported),
            ItemKindStaticAssert(sa) => self.static_assert(sa),
        }
    }

    pub fn static_assert(&mut self, sa: &static_assert) {
        let Some(v) = self.const_int_expr(&sa.cond) else { return };
        if v == 0 {
            let msg = match &sa.message {
                Some(m) => format!("static assertion failed: {m}"),
                None => "static assertion failed".to_string(),
            };
            diag::error("CR1314", sa.span, msg);
        }
    }

    fn check_export_allowed(&mut self, exported: Option<span>) -> bool {
        let Some(sp) = exported else { return false };
        if self.module.is_none() {
            diag::error("CR1409", sp, "only a `.crm` module can export")
                .help("move this code into a `.crm` module that declares `module <name>;`");
            return false;
        }
        true
    }

    fn import(&mut self, d: &import_decl) {
        if !self.kind.is_crust() {
            diag::error("CR1415", d.span, "a C file cannot import modules");
            return;
        }
        let iface = match self.env.resolve(&d.from) {
            Ok(i) => i,
            Err(msg) => {
                let code = if d.from.contains('/') { "CR1411" } else { "CR1404" };
                diag::error(code, d.from_span, msg);
                return;
            }
        };
        match &d.kind {
            ImportKindNamespace(alias, sp) => {
                if let Some(prev) = self.imported.get(alias) {
                    diag::error("CR1406", *sp, format!("`{alias}` is already imported"))
                        .secondary(*prev, "first imported here");
                }
                self.imported.insert(*alias, *sp);
                self.namespaces.insert(*alias, iface);
            }
            ImportKindNamed(names) => {
                for n in names {
                    let local = n.alias.unwrap_or(n.name);
                    let items: Vec<export_item> = iface.find(n.name).cloned().collect();
                    if items.is_empty() {
                        let mut list: Vec<String> =
                            iface.exports.iter().map(|e| e.name.to_string()).collect();
                        list.sort();
                        list.dedup();
                        diag::error(
                            "CR1405",
                            n.span,
                            format!("module `{}` does not export `{}`", iface.name, n.name),
                        )
                        .note(format!(
                            "its exports are: {}",
                            if list.is_empty() { "(none)".into() } else { list.join(", ") }
                        ));
                        continue;
                    }
                    if let Some(prev) = self.imported.get(&local) {
                        diag::error("CR1406", n.span, format!("`{local}` is already imported"))
                            .secondary(*prev, "first imported here");
                        continue;
                    }
                    self.imported.insert(local, n.span);
                    for e in items {
                        self.bind_import(local, &e, iface.name, n.span);
                    }
                }
            }
        }
    }

    /// Makes one exported item visible in file scope under `local`.
    pub fn bind_import(&mut self, local: sym, e: &export_item, module: sym, sp: span) {
        match &e.kind {
            ExportKindRecord(r) => {
                self.scopes[0].tags.insert(local, TagRecord(*r));
            }
            ExportKindEnum(en) => {
                self.scopes[0].tags.insert(local, TagEnum(*en));
            }
            ExportKindTypedef(t) => {
                self.scopes[0].ord.insert(local, BindingTypedef(*t));
            }
            ExportKindConst { ty, value } => {
                self.scopes[0].ord.insert(local, BindingConst(*value, *ty));
            }
            ExportKindFunc { .. } | ExportKindVar { .. } => {
                let g = self.import_global(e, module, sp);
                self.scopes[0].ord.insert(local, BindingGlobal(g));
            }
        }
    }

    pub fn import_global(&mut self, e: &export_item, module: sym, sp: span) -> global_id {
        let (symbol, ty, is_func) = match &e.kind {
            ExportKindFunc { symbol, ty, .. } => (symbol.clone(), *ty, true),
            ExportKindVar { symbol, ty, .. } => (symbol.clone(), *ty, false),
            _ => diag::ice("import_global on a non-object export"),
        };
        if let Some(i) =
            self.prog.globals.iter().position(|g| g.symbol == symbol && g.imported_from.is_some())
        {
            return i as global_id;
        }
        let mut g = self.new_global(e.name, ty, is_func, sp);
        g.symbol = symbol;
        g.imported_from = Some(module);
        g.linkage = LinkageExternal;
        match &e.kind {
            ExportKindFunc { unsafe_fn, safe, c_abi, noreturn, .. } => {
                g.unsafe_fn = *unsafe_fn;
                g.safe_attr = *safe;
                g.c_abi = *c_abi;
                g.noreturn = *noreturn;
            }
            ExportKindVar { needs_unsafe, c_abi, .. } => {
                g.needs_unsafe = *needs_unsafe;
                g.c_abi = *c_abi;
            }
            _ => {}
        }
        self.prog.globals.push(g);
        self.prog.globals.len() as global_id - 1
    }

    fn export_list(&mut self, l: &export_list, exported: Option<span>) {
        if !self.check_export_allowed(exported) {
            return;
        }
        match &l.from {
            None => {
                for n in &l.names {
                    if n.alias.is_some() {
                        let a = n.alias.unwrap();
                        self.export_alias(n.name, a, n.span);
                    } else {
                        self.export_names.push((n.name, n.span));
                    }
                }
            }
            Some((spec, sp)) => {
                let iface = match self.env.resolve(spec) {
                    Ok(i) => i,
                    Err(msg) => {
                        let code = if spec.contains('/') { "CR1411" } else { "CR1404" };
                        diag::error(code, *sp, msg);
                        return;
                    }
                };
                if l.star {
                    self.reexports.extend(iface.exports.iter().cloned());
                    return;
                }
                for n in &l.names {
                    let items: Vec<export_item> = iface.find(n.name).cloned().collect();
                    if items.is_empty() {
                        diag::error(
                            "CR1405",
                            n.span,
                            format!("module `{}` does not export `{}`", iface.name, n.name),
                        );
                    }
                    for mut e in items {
                        e.name = n.alias.unwrap_or(n.name);
                        self.reexports.push(e);
                    }
                }
            }
        }
    }

    /// `export { a as b };` of a local name: export it under another name.
    fn export_alias(&mut self, name: sym, alias: sym, sp: span) {
        self.export_names.push((name, sp));
        // The alias is resolved in collect_exports by renaming.
        self.export_renames.push((name, alias));
    }

    // ---- type resolution ----

    pub fn base_type(&mut self, spec: &decl_spec) -> qual_type {
        let t = match &spec.ty {
            TypeSpecNone => {
                if spec.quals == 0 && spec.storage.is_none() {
                    diag::error(
                        "CR1203",
                        spec.span,
                        "a type specifier is required; C23 has no implicit `int`",
                    );
                } else {
                    diag::error(
                        "CR1203",
                        spec.span,
                        "a type specifier is required; C23 has no implicit `int`",
                    );
                }
                q(error())
            }
            TypeSpecBuiltin(c) => q(self.builtin_type(c, spec.span)),
            TypeSpecRecord(r) => q(self.record_spec(r, &spec.attrs)),
            TypeSpecEnum(e) => q(self.enum_spec(e)),
            TypeSpecTypedefName(name, ns, sp) => self.typedef_type(*name, *ns, *sp),
            TypeSpecTypeofExpr(e, unqual) => {
                self.unevaluated += 1;
                let te = self.expr(e);
                self.unevaluated -= 1;
                if *unqual { te.ty.unqual() } else { te.ty }
            }
            TypeSpecTypeofType(tn, unqual) => {
                let t = self.type_name(tn);
                if *unqual { t.unqual() } else { t }
            }
            TypeSpecAtomic(tn) => self.type_name(tn).with(QualAtomic),
            TypeSpecAuto => {
                diag::error(
                    "CR1322",
                    spec.span,
                    "`auto` needs an initializer to infer the type from",
                );
                q(error())
            }
            TypeSpecRef { mutable, lifetime, inner } => {
                let p = self.type_name(inner);
                self.check_borrow_allowed(spec.span);
                if is_function(p.ty) || is_void(p.ty) {
                    diag::error(
                        "CR1321",
                        spec.span,
                        format!("cannot borrow a value of type `{}`", display(p)),
                    );
                }
                q(intern(TypeKindRef {
                    mutable: *mutable,
                    lifetime: lifetime.map(|l| l.0),
                    pointee: p,
                }))
            }
            TypeSpecSlice { mutable, lifetime, elem } => {
                let e = self.type_name(elem);
                self.check_borrow_allowed(spec.span);
                if !is_complete(e.ty) {
                    diag::error(
                        "CR1310",
                        spec.span,
                        format!("slice element type `{}` is incomplete", display(e)),
                    );
                }
                q(intern(TypeKindSlice {
                    mutable: *mutable,
                    lifetime: lifetime.map(|l| l.0),
                    elem: e,
                }))
            }
        };
        t.with(spec.quals)
    }

    fn check_borrow_allowed(&self, sp: span) {
        if !self.is_crust_span(sp) {
            diag::error("CR1321", sp, "borrows exist only in Crust files");
        }
    }

    fn builtin_type(&mut self, c: &builtin_counts, sp: span) -> type_id {
        let bad = |what: &str| {
            diag::error("CR1202", sp, format!("invalid type specifier combination: {what}"));
            error()
        };
        if c.complex > 0 {
            diag::error("CR1301", sp, "complex types are not supported in Crust 0.1.0");
            return error();
        }
        if c.va_list > 0 {
            return self.va_list_type();
        }
        if c.void > 0 {
            return void();
        }
        if c.bool_ > 0 {
            return bool_();
        }
        if c.float > 0 {
            return float_of(FloatKindFloat);
        }
        if c.double > 0 {
            return float_of(if c.long > 0 { FloatKindLongDouble } else { FloatKindDouble });
        }
        if c.signed > 0 && c.unsigned > 0 {
            return bad("`signed` and `unsigned`");
        }
        let u = c.unsigned > 0;
        let k = if c.char_ > 0 {
            if u {
                IntKindUChar
            } else if c.signed > 0 {
                IntKindSChar
            } else {
                IntKindChar
            }
        } else if c.int128 > 0 {
            if u { IntKindUInt128 } else { IntKindInt128 }
        } else if c.short > 0 {
            if u { IntKindUShort } else { IntKindShort }
        } else if c.long >= 2 {
            if u { IntKindULongLong } else { IntKindLongLong }
        } else if c.long == 1 {
            if u { IntKindULong } else { IntKindLong }
        } else if u {
            IntKindUInt
        } else {
            IntKindInt
        };
        int_of(k)
    }

    fn typedef_type(&mut self, name: sym, ns: Option<sym>, sp: span) -> qual_type {
        if let Some(ns) = ns {
            if let Some(iface) = self.namespaces.get(&ns) {
                if let Some(t) = iface.find(name).find_map(|e| match e.kind {
                    ExportKindTypedef(t) => Some(t),
                    _ => None,
                }) {
                    return t;
                }
            }
            diag::error("CR1203", sp, format!("unknown type name `{ns}::{name}`"));
            return q(error());
        }
        match self.lookup(name) {
            Some(BindingTypedef(t)) => t,
            _ => {
                diag::error("CR1203", sp, format!("unknown type name `{name}`"));
                q(error())
            }
        }
    }

    pub fn type_name(&mut self, tn: &type_name) -> qual_type {
        let base = self.base_type(&tn.spec);
        self.apply_declarator(base, &tn.declarator, false).0
    }

    /// Applies a declarator to a base type. Returns the type and, for a
    /// function declarator, its parameter names and spans.
    pub fn apply_declarator(
        &mut self,
        base: qual_type,
        d: &declarator,
        is_param: bool,
    ) -> (qual_type, Vec<(Option<sym>, span)>) {
        let mut t = base;
        let mut pnames = Vec::new();
        let n = d.derived.len();
        for (i, der) in d.derived.iter().enumerate() {
            match der {
                DerivedPointer { quals } => {
                    t = qual_type { ty: pointer_to(t), quals: *quals };
                }
                DerivedArray { size, quals, star, span: asp, .. } => {
                    if is_function(t.ty) {
                        diag::error("CR1315", *asp, "array of functions is not allowed");
                        t = q(error());
                        continue;
                    }
                    if is_ref(t.ty) || is_slice(t.ty) {
                        diag::error(
                            "CR1321",
                            *asp,
                            "arrays of borrows are not supported in Crust 0.1.0",
                        );
                    }
                    let len = match size {
                        None => {
                            if *star {
                                diag::error(
                                    "CR1301",
                                    *asp,
                                    "variable-length arrays are not supported in Crust 0.1.0",
                                );
                            }
                            None
                        }
                        Some(e) => match self.const_int_expr_quiet(e) {
                            Some(v) if v >= 0 => Some(v as u64),
                            Some(_) => {
                                diag::error("CR1308", e.span, "array size is negative");
                                Some(0)
                            }
                            None => {
                                diag::error(
                                    "CR1301",
                                    e.span,
                                    "variable-length arrays are not supported in Crust 0.1.0",
                                )
                                .note("the array size must be an integer constant expression");
                                Some(1)
                            }
                        },
                    };
                    if !is_complete(t.ty) && !is_error(t.ty) {
                        diag::error(
                            "CR1310",
                            *asp,
                            format!("array element type `{}` is incomplete", display(t)),
                        );
                    }
                    let elem = t;
                    t = qual_type { ty: array_of(elem, len), quals: 0 };
                    // In a parameter, the outermost array adjusts to a pointer.
                    if is_param && i == n - 1 {
                        t = qual_type { ty: pointer_to(elem), quals: *quals };
                    }
                }
                DerivedFunction { params, variadic, span: fsp } => {
                    if is_function(t.ty) || is_array(t.ty) {
                        diag::error(
                            "CR1315",
                            *fsp,
                            "a function cannot return a function or an array",
                        );
                    }
                    let mut ps = Vec::new();
                    pnames.clear();
                    self.push_scope();
                    for p in params {
                        let base = self.base_type(&p.spec);
                        let (pt, _) = self.apply_declarator(base, &p.declarator, true);
                        let pt = self.adjust_param(pt);
                        if is_void(pt.ty) {
                            diag::error("CR1319", p.span, "a parameter cannot have type `void`");
                        }
                        ps.push(pt);
                        pnames.push((
                            p.declarator.name.map(|n| n.0),
                            p.declarator.name.map(|n| n.1).unwrap_or(p.span),
                        ));
                    }
                    self.pop_scope();
                    let ft = func_type { ret: t.unqual(), params: ps, variadic: *variadic };
                    t = q(intern(TypeKindFunction(ft)));
                }
            }
        }
        if is_param {
            t = self.adjust_param(t);
        }
        (t, pnames)
    }

    pub fn adjust_param(&mut self, t: qual_type) -> qual_type {
        match t.kind() {
            TypeKindArray(e, _) => q(pointer_to(e)),
            TypeKindFunction(_) => q(pointer_to(t)),
            _ => t,
        }
    }

    // ---- globals ----

    pub fn new_global(&self, name: sym, ty: qual_type, is_func: bool, sp: span) -> global_decl {
        global_decl {
            name,
            ty,
            is_func,
            linkage: LinkageExternal,
            symbol: name.to_string(),
            defined: false,
            init: None,
            tentative: false,
            span: sp,
            c_origin: !self.is_crust_span(sp),
            safe_attr: false,
            unsafe_fn: false,
            c_abi: false,
            exported: false,
            needs_unsafe: false,
            thread_local: false,
            noreturn: false,
            inline_: false,
            imported_from: None,
            const_value: None,
            is_constexpr: false,
            is_let: false,
            used: false,
        }
    }

    /// Declares (or redeclares) a file-scope function or variable.
    pub fn declare_global(
        &mut self,
        name: sym,
        ty: qual_type,
        is_func: bool,
        sp: span,
        internal: bool,
    ) -> global_id {
        let existing = match self.scopes[0].ord.get(&name) {
            Some(BindingGlobal(g)) => Some(*g),
            Some(_) => {
                diag::error(
                    "CR1303",
                    sp,
                    format!("`{name}` redeclared as a different kind of symbol"),
                );
                None
            }
            None => self
                .prog
                .globals
                .iter()
                .position(|g| {
                    g.name == name && g.imported_from.is_none() && g.symbol == name.as_str()
                })
                .map(|i| i as global_id)
                .filter(|_| false),
        };
        if let Some(g) = existing {
            let old = self.prog.globals[g as usize].clone();
            if old.imported_from.is_some() {
                diag::error(
                    "CR1406",
                    sp,
                    format!("`{name}` is imported and cannot be redeclared here"),
                )
                .secondary(old.span, "imported here");
                return g;
            }
            if old.is_func != is_func || !compatible(strip_lifetimes(old.ty), strip_lifetimes(ty)) {
                self.report_conflict(name, &old, ty, sp);
            } else if self.kind == FileKindCr && is_func {
                self.check_pair(name, &old, ty, sp);
            }
            let gd = &mut self.prog.globals[g as usize];
            // Prefer the declaration with complete information.
            if let (TypeKindArray(_, None), TypeKindArray(_, Some(_))) = (old.ty.kind(), ty.kind())
            {
                gd.ty = ty;
            }
            if internal {
                gd.linkage = LinkageInternal;
            }
            return g;
        }
        let mut gd = self.new_global(name, ty, is_func, sp);
        if internal {
            gd.linkage = LinkageInternal;
        }
        self.prog.globals.push(gd);
        let g = self.prog.globals.len() as global_id - 1;
        self.scopes[0].ord.insert(name, BindingGlobal(g));
        g
    }

    fn report_conflict(&self, name: sym, old: &global_decl, ty: qual_type, sp: span) {
        let hr = diag::file_kind(old.span.file) == FileKindHr && self.kind == FileKindCr;
        let code = if hr { "CR1501" } else { "CR1303" };
        let msg = if hr {
            format!("definition of `{name}` does not match its declaration in the header")
        } else {
            format!("conflicting types for `{name}`")
        };
        diag::error(code, sp, msg)
            .label(format!("has type `{}`", display_decl(ty, name.as_str())))
            .secondary(
                old.span,
                format!("previously declared as `{}`", display_decl(old.ty, name.as_str())),
            );
    }

    /// D-035: a `.cr` definition must match its `.hr` declaration exactly.
    fn check_pair(&self, name: sym, old: &global_decl, ty: qual_type, sp: span) {
        if diag::file_kind(old.span.file) != FileKindHr || diag::file_kind(sp.file) != FileKindCr {
            return;
        }
        let norm = |t: qual_type| -> qual_type {
            match t.kind() {
                TypeKindFunction(f) => q(intern(TypeKindFunction(func_type {
                    ret: f.ret,
                    params: f.params.iter().map(|p| p.unqual()).collect(),
                    variadic: f.variadic,
                }))),
                _ => t,
            }
        };
        if norm(old.ty) != norm(ty) {
            self.report_conflict(name, old, ty, sp);
        }
    }

    fn declaration(&mut self, d: &declaration, exported: Option<span>) {
        let exported = if exported.is_some() {
            self.check_export_allowed(exported).then(|| exported.unwrap())
        } else {
            None
        };
        self.validate_attrs(&d.spec.attrs);
        if d.inits.is_empty() {
            let _ = self.base_type(&d.spec);
            if let Some(sp) = exported {
                match &d.spec.ty {
                    TypeSpecRecord(r) if r.tag.is_some() => {
                        self.export_names.push((r.tag.unwrap().0, sp))
                    }
                    TypeSpecEnum(e) => {
                        if let Some(t) = e.tag {
                            self.export_names.push((t.0, sp));
                        }
                        for en in e.enumerators.iter().flatten() {
                            self.export_names.push((en.name, sp));
                        }
                    }
                    _ => {
                        diag::error(
                            "CR1409",
                            sp,
                            "nothing to export: this declaration declares no name",
                        );
                    }
                }
            }
            return;
        }
        let base = self.base_type(&d.spec);
        if let (Some(sp), TypeSpecEnum(e)) = (exported, &d.spec.ty) {
            for en in e.enumerators.iter().flatten() {
                self.export_names.push((en.name, sp));
            }
        }
        for id in &d.inits {
            self.file_declarator(&d.spec, base, id, exported);
        }
    }

    fn validate_attrs(&self, attrs: &[attribute]) {
        for a in attrs {
            if a.prefix == Some(SymCrust) && !matches!(a.name, SymDrop | SymSafe | SymCAbi) {
                diag::error("CR1320", a.span, format!("unknown attribute `crust::{}`", a.name))
                    .note("Crust attributes are `crust::drop`, `crust::safe`, and `crust::c_abi`");
            }
        }
    }

    fn has_attr(spec: &decl_spec, d: &declarator, name: sym) -> Option<span> {
        spec.attrs.iter().chain(d.attrs.iter()).find(|a| a.is_crust(name)).map(|a| a.span)
    }

    fn file_declarator(
        &mut self,
        spec: &decl_spec,
        base: qual_type,
        id: &init_declarator,
        exported: Option<span>,
    ) {
        let d = &id.declarator;
        let Some((name, nsp)) = d.name else { return };
        let (ty, _) = self.apply_declarator(base, d, false);
        let storage = spec.storage.map(|s| s.0);
        if storage == Some(StorageTypedef) {
            if let Some(Some(BindingTypedef(old))) =
                self.scopes.last().map(|s| s.ord.get(&name).cloned())
            {
                if !compatible(old, ty) {
                    diag::error(
                        "CR1303",
                        nsp,
                        format!("typedef `{name}` redefined with a different type"),
                    );
                }
            }
            if self.at_file_scope() {
                if let Some(sp) = exported {
                    self.export_names.push((name, sp));
                }
            }
            self.scopes.last_mut().unwrap().ord.insert(name, BindingTypedef(ty));
            return;
        }
        if is_ref(ty.ty) || is_slice(ty.ty) {
            diag::error("CR1321", nsp, "a global variable cannot hold a borrow in Crust 0.1.0");
        }
        let is_func = is_function(ty.ty);
        let internal = storage == Some(StorageStatic);
        let g = self.declare_global(name, ty, is_func, nsp, internal);
        self.apply_decl_flags(g, spec, d, exported);
        if let Some(label) = &id.asm_label {
            self.prog.globals[g as usize].symbol = label.clone();
        }
        if is_func {
            if id.init.is_some() {
                diag::error("CR1311", nsp, format!("function `{name}` cannot have an initializer"));
            }
            return;
        }
        if spec.thread_local {
            self.prog.globals[g as usize].thread_local = true;
        }
        let is_extern = storage == Some(StorageExtern);
        match &id.init {
            Some(init) => {
                if self.prog.globals[g as usize].defined {
                    diag::error("CR1303", nsp, format!("redefinition of `{name}`"));
                }
                self.prog.globals[g as usize].defined = true;
                self.prog.globals[g as usize].tentative = false;
                self.global_init(g, init, spec.constexpr_);
            }
            None => {
                if spec.constexpr_ {
                    diag::error("CR1308", nsp, "a `constexpr` object needs an initializer");
                }
                if !is_extern && !self.prog.globals[g as usize].defined {
                    self.prog.globals[g as usize].tentative = true;
                }
            }
        }
    }

    fn apply_decl_flags(
        &mut self,
        g: global_id,
        spec: &decl_spec,
        d: &declarator,
        exported: Option<span>,
    ) {
        let safe = Self::has_attr(spec, d, SymSafe);
        let c_abi = Self::has_attr(spec, d, SymCAbi);
        let gd = &mut self.prog.globals[g as usize];
        if let Some(sp) = safe {
            if !gd.is_func {
                diag::error("CR1320", sp, "`crust::safe` applies only to function declarations");
            }
            gd.safe_attr = true;
        }
        if spec.unsafe_.is_some() {
            gd.unsafe_fn = true;
        }
        if spec.noreturn
            || spec
                .attrs
                .iter()
                .any(|a| a.prefix.is_none() && a.name.as_str().trim_matches('_') == "noreturn")
        {
            gd.noreturn = true;
        }
        if spec.inline_ {
            gd.inline_ = true;
        }
        if spec.constexpr_ {
            gd.is_constexpr = true;
        }
        if let Some(sp) = c_abi {
            if exported.is_none() {
                diag::error("CR1320", sp, "`crust::c_abi` applies only to exported declarations")
                    .help("write `[[crust::c_abi]] export ...`");
            }
            gd.c_abi = true;
        }
        if let Some(sp) = exported {
            gd.exported = true;
            let n = gd.name;
            self.export_names.push((n, sp));
        }
    }

    fn file_let(&mut self, l: &let_decl, exported: Option<span>) {
        let exported = if exported.is_some() {
            self.check_export_allowed(exported).then(|| exported.unwrap())
        } else {
            None
        };
        let Some(init) = &l.init else {
            diag::error("CR1413", l.span, "a module-level `let` needs a constant initializer");
            return;
        };
        let ty = match &l.ty {
            Some(t) => self.type_name(t),
            None => match init {
                InitializerExpr(e) => {
                    self.unevaluated += 1;
                    let te = self.expr(e);
                    self.unevaluated -= 1;
                    self.decay(te).ty.unqual()
                }
                InitializerList(_, sp) => {
                    diag::error(
                        "CR1322",
                        *sp,
                        "a braced initializer needs a type: write `let name: T = { ... };`",
                    );
                    q(error())
                }
            },
        };
        if is_ref(ty.ty) || is_slice(ty.ty) {
            diag::error(
                "CR1321",
                l.name_span,
                "a global variable cannot hold a borrow in Crust 0.1.0",
            );
        }
        let ty = if l.mutable { ty } else { ty.with(QualConst) };
        let g = self.declare_global(l.name, ty, false, l.name_span, false);
        {
            let gd = &mut self.prog.globals[g as usize];
            gd.is_let = true;
            gd.defined = true;
            gd.needs_unsafe = l.mutable;
            if self.module.is_some() && exported.is_none() {
                gd.linkage = LinkageInternal;
            }
            if let Some(sp) = exported {
                gd.exported = true;
                self.export_names.push((l.name, sp));
            }
        }
        self.global_init(g, init, !l.mutable);
    }

    fn function_def(&mut self, f: &function_def, exported: Option<span>) {
        let exported = if exported.is_some() {
            self.check_export_allowed(exported).then(|| exported.unwrap())
        } else {
            None
        };
        self.validate_attrs(&f.spec.attrs);
        let d = &f.declarator;
        let Some((name, nsp)) = d.name else { return };
        let base = self.base_type(&f.spec);
        let (ty, pnames) = self.apply_declarator(base, d, false);
        let Some(ft) = func_of(ty.ty).filter(|_| is_function(ty.ty)) else {
            diag::error(
                "CR1315",
                nsp,
                format!("`{name}` is defined with a body but is not a function"),
            );
            return;
        };
        let internal = matches!(f.spec.storage, Some((StorageStatic, _)));
        let g = self.declare_global(name, ty, true, nsp, internal);
        self.apply_decl_flags(g, &f.spec, d, exported);
        if self.prog.globals[g as usize].defined {
            diag::error("CR1303", nsp, format!("redefinition of `{name}`"))
                .secondary(self.prog.globals[g as usize].span, "first defined here");
        }
        {
            let gd = &mut self.prog.globals[g as usize];
            gd.defined = true;
            gd.span = nsp;
            gd.c_origin = !diag::file_kind(nsp.file).is_crust();
        }
        let crust = self.is_crust_span(f.span);
        let unsafe_fn = self.prog.globals[g as usize].unsafe_fn;
        self.fctx = Some(func_ctx {
            locals: Vec::new(),
            ret: ft.ret,
            labels: Vec::new(),
            label_map: HashMap::new(),
            loops: 0,
            breakable: 0,
            switches: Vec::new(),
            unsafe_depth: 0,
            unsafe_fn,
            crust,
            blocks: vec![Vec::new()],
            name,
            static_count: 0,
        });
        self.push_scope();
        let mut params = Vec::new();
        for (i, pt) in ft.params.iter().enumerate() {
            let (pname, psp) = pnames.get(i).cloned().unwrap_or((None, nsp));
            let lid = self.new_local(pname.unwrap_or(SymEmpty), *pt, psp, true, false);
            self.fctx.as_mut().unwrap().locals[lid as usize].is_param = true;
            if let Some(n) = pname {
                self.scopes.last_mut().unwrap().ord.insert(n, BindingLocal(lid));
            }
            params.push(lid);
        }
        self.fctx.as_mut().unwrap().blocks.last_mut().unwrap().clear();
        let body = match &f.body.kind {
            StmtKindCompound(items) => self.block(items, f.body.span, false, false),
            _ => diag::ice("function body is not a compound statement"),
        };
        self.pop_scope();
        let fc = self.fctx.take().unwrap();
        for l in &fc.labels {
            if !l.defined {
                diag::error("CR1312", l.span, format!("use of undeclared label `{}`", l.name));
            }
        }
        self.prog.functions.push(func_body {
            global: g,
            params,
            locals: fc.locals,
            body,
            span: f.span,
            crust,
            unsafe_fn,
            labels: fc.labels,
            ret: ft.ret,
            name,
        });
    }

    pub fn new_local(
        &mut self,
        name: sym,
        ty: qual_type,
        sp: span,
        mutable: bool,
        is_let: bool,
    ) -> local_id {
        let fc = self.fctx.as_mut().expect("local outside a function");
        fc.locals.push(local_info {
            name,
            ty,
            span: sp,
            mutable,
            is_let,
            is_param: false,
            is_temp: false,
        });
        let id = fc.locals.len() as local_id - 1;
        fc.blocks.last_mut().unwrap().push(id);
        id
    }

    // ---- end of unit ----

    fn finalize(&mut self) {
        // Resolve drop functions.
        for pd in std::mem::take(&mut self.pending_drops) {
            match self.scopes[0].ord.get(&pd.func).cloned() {
                Some(BindingGlobal(g)) if self.prog.globals[g as usize].is_func => {
                    let gd = self.prog.globals[g as usize].clone();
                    let ft = func_of(gd.ty.ty).unwrap();
                    let rt = intern(TypeKindRecord(pd.record));
                    let ok_param = ft.params.len() == 1
                        && match ft.params[0].kind() {
                            TypeKindPointer(p) => p.ty == rt,
                            TypeKindRef { mutable: true, pointee, .. } => pointee.ty == rt,
                            _ => false,
                        };
                    if !ok_param || !is_void(ft.ret.ty) {
                        diag::error(
                            "CR1320",
                            pd.span,
                            format!(
                                "drop function `{}` must have type `void (struct T *)`",
                                pd.func
                            ),
                        )
                        .secondary(gd.span, format!("`{}` is declared here", pd.func));
                        continue;
                    }
                    self.prog.globals[g as usize].used = true;
                    let symbol_global = g;
                    update_record(pd.record, |r| {
                        r.drop = Some(drop_info {
                            symbol: format!("#global{symbol_global}"),
                            name: pd.func,
                        })
                    });
                }
                _ => {
                    diag::error(
                        "CR1320",
                        pd.span,
                        format!("drop function `{}` is not declared", pd.func),
                    )
                    .help("declare it at file scope: `void name(struct T *value);`");
                }
            }
        }
        // Symbols and linkage.
        let module = self.module;
        for g in self.prog.globals.iter_mut() {
            if g.imported_from.is_some() {
                continue;
            }
            let in_module_file = g.span.file == self.main_file && module.is_some();
            if !in_module_file || g.symbol != g.name.as_str() {
                if g.tentative && !g.defined {
                    g.defined = true;
                }
                continue;
            }
            let m = module.unwrap();
            if g.c_abi {
                g.linkage = LinkageExternal;
            } else if g.is_func && g.name == SymMain && !g.exported {
                g.linkage = LinkageExternal;
            } else if g.exported {
                g.symbol = mangle(m, g.name);
                g.linkage = LinkageExternal;
            } else if g.defined || g.tentative {
                g.symbol = mangle(m, g.name);
                g.linkage = LinkageInternal;
            } else {
                // An undefined prototype in a module names a C function.
                g.c_origin = !g.safe_attr;
            }
            if g.tentative && !g.defined {
                g.defined = true;
            }
        }
        // Drop function references now have final symbols.
        for (tag, t) in self.file_tags.clone() {
            let _ = tag;
            if let TagRecord(r) = t {
                let d = record(r).drop;
                if let Some(di) = d {
                    if let Some(idx) = di.symbol.strip_prefix("#global") {
                        let g: usize = idx.parse().unwrap();
                        let symbol = self.prog.globals[g].symbol.clone();
                        update_record(r, |r| r.drop.as_mut().unwrap().symbol = symbol);
                    }
                }
            }
        }
        // Record drop references for records defined in block scopes too.
        for g in 0..self.prog.globals.len() {
            let _ = g;
        }
    }

    fn collect_exports(&mut self) -> Vec<export_item> {
        let mut out = Vec::new();
        let names = std::mem::take(&mut self.export_names);
        let renames = std::mem::take(&mut self.export_renames);
        let mut seen: Vec<(sym, bool)> = Vec::new();
        for (name, sp) in names {
            let exported_as =
                renames.iter().find(|(n, _)| *n == name).map(|(_, a)| *a).unwrap_or(name);
            let mut found = false;
            if let Some(t) = self.scopes[0].tags.get(&name).copied() {
                found = true;
                let kind = match t {
                    TagRecord(r) => ExportKindRecord(r),
                    TagEnum(e) => ExportKindEnum(e),
                };
                if !seen.contains(&(exported_as, true)) {
                    seen.push((exported_as, true));
                    out.push(export_item { name: exported_as, kind });
                }
            }
            if let Some(o) = self.scopes[0].ord.get(&name).cloned() {
                if self.imported.contains_key(&name) {
                    diag::error(
                        "CR1408",
                        sp,
                        format!(
                            "`{name}` is imported; re-export it with `export {{ {name} }} from \"<module>\";`"
                        ),
                    );
                    continue;
                }
                found = true;
                let kind = match o {
                    BindingTypedef(t) => ExportKindTypedef(t),
                    BindingConst(v, t) => ExportKindConst { ty: t, value: v },
                    BindingGlobal(g) => {
                        let gd = &mut self.prog.globals[g as usize];
                        gd.exported = true;
                        if gd.is_constexpr && gd.const_value.is_some() {
                            ExportKindConst { ty: gd.ty, value: gd.const_value.unwrap() }
                        } else if gd.is_func {
                            ExportKindFunc {
                                symbol: gd.symbol.clone(),
                                ty: gd.ty,
                                unsafe_fn: gd.unsafe_fn,
                                safe: gd.safe_attr,
                                c_abi: gd.c_abi,
                                noreturn: gd.noreturn,
                            }
                        } else {
                            ExportKindVar {
                                symbol: gd.symbol.clone(),
                                ty: gd.ty,
                                needs_unsafe: gd.needs_unsafe,
                                c_abi: gd.c_abi,
                            }
                        }
                    }
                    BindingLocal(_) => continue,
                };
                if let BindingGlobal(g) = o {
                    let gd = &self.prog.globals[g as usize];
                    if diag::file_kind(gd.span.file) != FileKindCrm
                        || gd.span.file != self.main_file
                    {
                        diag::error(
                            "CR1408",
                            sp,
                            format!("`{name}` is declared in an included header and cannot be exported"),
                        )
                        .help("C declarations are not re-exported; write an exported wrapper function");
                        continue;
                    }
                    if gd.c_abi {
                        self.check_c_abi(gd.ty, gd.span, name);
                    }
                    if !gd.defined && !gd.is_func {
                        diag::error(
                            "CR1408",
                            sp,
                            format!("exported variable `{name}` is never defined"),
                        );
                    }
                }
                if !seen.contains(&(exported_as, false)) {
                    seen.push((exported_as, false));
                    out.push(export_item { name: exported_as, kind });
                }
            }
            if !found {
                diag::error(
                    "CR1302",
                    sp,
                    format!("cannot export `{name}`: it is not declared in this module"),
                );
            }
        }
        for g in &self.prog.globals {
            if g.exported && g.is_func && !g.defined && g.imported_from.is_none() {
                diag::error(
                    "CR1408",
                    g.span,
                    format!("exported function `{}` is never defined in this module", g.name),
                );
            }
        }
        for e in std::mem::take(&mut self.reexports) {
            let tag = e.kind.is_tag();
            if seen.contains(&(e.name, tag)) {
                continue;
            }
            seen.push((e.name, tag));
            out.push(e);
        }
        out
    }

    /// A `[[crust::c_abi]]` export may use only C-compatible types (D-034).
    fn check_c_abi(&self, t: qual_type, sp: span, name: sym) {
        let mut bad: Option<String> = None;
        let mut check = |t: qual_type| {
            if bad.is_some() {
                return;
            }
            match t.kind() {
                TypeKindRef { pointee, .. } => {
                    bad = Some(format!(
                        "a borrow `{}`; use `{}` instead",
                        display(t),
                        display(q(pointer_to(pointee)))
                    ))
                }
                TypeKindSlice { elem, .. } => {
                    bad = Some(format!(
                        "a slice `{}`; pass a `{}` and a `size_t` length instead",
                        display(t),
                        display(q(pointer_to(elem)))
                    ))
                }
                _ if is_owned(t.ty) => {
                    bad = Some(format!(
                        "the owned type `{}` by value; pass `{}` instead",
                        display(t),
                        display(q(pointer_to(t)))
                    ))
                }
                _ => {}
            }
        };
        match t.kind() {
            TypeKindFunction(f) => {
                check(f.ret);
                for p in f.params {
                    check(p);
                }
            }
            _ => check(t),
        }
        if let Some(b) = bad {
            diag::error("CR1412", sp, format!("`[[crust::c_abi]]` export `{name}` uses {b}"));
        }
    }
}

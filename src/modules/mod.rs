/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
pub mod header;
pub mod interface;

use crate::lex::sym;
use crate::sema::tir::ConstValue;
use crate::sema::types::qual_type;
use std::rc::Rc;

/// One exported name of a module, with everything an importer needs.
#[derive(Clone, Debug)]
pub struct export_item {
    pub name: sym,
    pub kind: ExportKind,
}

#[derive(Clone, Debug)]
pub enum ExportKind {
    ExportKindFunc {
        symbol: String,
        ty: qual_type,
        unsafe_fn: bool,
        safe: bool,
        c_abi: bool,
        noreturn: bool,
    },
    ExportKindVar {
        symbol: String,
        ty: qual_type,
        needs_unsafe: bool,
        c_abi: bool,
    },
    ExportKindConst {
        ty: qual_type,
        value: ConstValue,
    },
    ExportKindTypedef(qual_type),
    ExportKindRecord(u32),
    ExportKindEnum(u32),
}
pub use ExportKind::*;

impl ExportKind {
    /// Whether this export lives in the tag namespace (`struct x`, `enum x`).
    pub fn is_tag(&self) -> bool {
        matches!(self, ExportKindRecord(_) | ExportKindEnum(_))
    }

    pub fn describe(&self) -> &'static str {
        match self {
            ExportKindFunc { .. } => "function",
            ExportKindVar { .. } => "variable",
            ExportKindConst { .. } => "constant",
            ExportKindTypedef(_) => "typedef",
            ExportKindRecord(_) => "struct or union",
            ExportKindEnum(_) => "enum",
        }
    }
}

/// The interface of a compiled module: what a `.crmi` file holds.
#[derive(Clone, Debug)]
pub struct module_interface {
    pub name: sym,
    pub hash: u64,
    pub exports: Vec<export_item>,
}

impl module_interface {
    pub fn find(&self, name: sym) -> impl Iterator<Item = &export_item> {
        self.exports.iter().filter(move |e| e.name == name)
    }

    pub fn is_typedef(&self, name: sym) -> bool {
        self.find(name).any(|e| matches!(e.kind, ExportKindTypedef(_)))
    }
}

/// Resolves import specifiers to module interfaces.
pub trait module_env {
    fn resolve(&self, spec: &str) -> Result<Rc<module_interface>, String>;
}

pub struct empty_env;

impl module_env for empty_env {
    fn resolve(&self, spec: &str) -> Result<Rc<module_interface>, String> {
        Err(format!("no module named `{spec}` is part of this build"))
    }
}

/// The mangled symbol for a Crust-ABI declaration of a module.
pub fn mangle(module: sym, name: sym) -> String {
    format!("_CR.{module}.{name}")
}

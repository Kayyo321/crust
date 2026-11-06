/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use crate::diag::{FileKind, span};
use crate::lex::{sym, token};
use std::rc::Rc;

/// One parsed translation unit: a `.c`, `.cr`, or `.crm` file with its
/// includes expanded.
pub struct translation_unit {
    pub file: u32,
    pub kind: FileKind,
    pub module: Option<(sym, span)>,
    pub items: Vec<item>,
}

pub struct item {
    pub kind: ItemKind,
    pub span: span,
    /// `export` was written before the item.
    pub exported: Option<span>,
}

pub enum ItemKind {
    ItemKindDecl(declaration),
    ItemKindFunc(function_def),
    ItemKindLet(let_decl),
    ItemKindStaticAssert(static_assert),
    ItemKindImport(import_decl),
    ItemKindExportList(export_list),
}
pub use ItemKind::*;

#[derive(Clone, Debug)]
pub struct attribute {
    pub prefix: Option<sym>,
    pub name: sym,
    pub args: Option<Vec<token>>,
    pub span: span,
}

impl attribute {
    pub fn is_crust(&self, name: sym) -> bool {
        self.prefix == Some(crate::lex::SymCrust) && self.name == name
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Storage {
    StorageTypedef,
    StorageExtern,
    StorageStatic,
    StorageAuto,
    StorageRegister,
}
pub use Storage::*;

pub const QualConst: u8 = 1;
pub const QualVolatile: u8 = 2;
pub const QualRestrict: u8 = 4;
pub const QualAtomic: u8 = 8;

#[derive(Clone, Default, Debug)]
pub struct builtin_counts {
    pub void: u8,
    pub bool_: u8,
    pub char_: u8,
    pub short: u8,
    pub int: u8,
    pub long: u8,
    pub float: u8,
    pub double: u8,
    pub signed: u8,
    pub unsigned: u8,
    pub int128: u8,
    pub complex: u8,
    pub va_list: u8,
}

impl builtin_counts {
    pub fn any(&self) -> bool {
        let b = self;
        b.void
            + b.bool_
            + b.char_
            + b.short
            + b.int
            + b.long
            + b.float
            + b.double
            + b.signed
            + b.unsigned
            + b.int128
            + b.complex
            + b.va_list
            > 0
    }
}

#[derive(Clone, Debug)]
pub enum TypeSpec {
    TypeSpecNone,
    TypeSpecBuiltin(builtin_counts),
    TypeSpecRecord(Rc<record_spec>),
    TypeSpecEnum(Rc<enum_spec>),
    TypeSpecTypedefName(sym, Option<sym>, span),
    TypeSpecTypeofExpr(Box<expr>, bool),
    TypeSpecTypeofType(Box<type_name>, bool),
    TypeSpecAtomic(Box<type_name>),
    TypeSpecAuto,
    /// `ref ['a] [mut] T`
    TypeSpecRef {
        mutable: bool,
        lifetime: Option<(sym, span)>,
        inner: Box<type_name>,
    },
    /// `ref ['a] [mut] [T]`
    TypeSpecSlice {
        mutable: bool,
        lifetime: Option<(sym, span)>,
        elem: Box<type_name>,
    },
}
pub use TypeSpec::*;

#[derive(Clone, Debug)]
pub struct decl_spec {
    pub storage: Option<(Storage, span)>,
    pub thread_local: bool,
    pub inline_: bool,
    pub noreturn: bool,
    pub constexpr_: bool,
    pub unsafe_: Option<span>,
    pub ty: TypeSpec,
    pub quals: u8,
    pub attrs: Vec<attribute>,
    pub align: Vec<align_spec>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub enum align_spec {
    AlignType(Box<type_name>),
    AlignExpr(Box<expr>),
}

#[derive(Clone, Debug)]
pub struct record_spec {
    pub is_union: bool,
    pub tag: Option<(sym, span)>,
    pub ns: Option<sym>,
    pub fields: Option<Vec<field_decl>>,
    pub attrs: Vec<attribute>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct field_decl {
    pub spec: decl_spec,
    /// Each member declarator, with its bit-field width.
    pub declarators: Vec<(Option<declarator>, Option<expr>)>,
    pub span: span,
    pub static_assert: Option<static_assert>,
}

#[derive(Clone, Debug)]
pub struct enum_spec {
    pub tag: Option<(sym, span)>,
    pub ns: Option<sym>,
    pub fixed: Option<Box<type_name>>,
    pub enumerators: Option<Vec<enumerator>>,
    pub attrs: Vec<attribute>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct enumerator {
    pub name: sym,
    pub span: span,
    pub value: Option<expr>,
}

/// Derived declarator parts, in the order they apply to the base type.
#[derive(Clone, Debug)]
pub enum Derived {
    DerivedPointer { quals: u8 },
    DerivedArray { size: Option<Box<expr>>, is_static: bool, quals: u8, star: bool, span: span },
    DerivedFunction { params: Vec<param_decl>, variadic: bool, span: span },
}
pub use Derived::*;

#[derive(Clone, Debug)]
pub struct declarator {
    pub name: Option<(sym, span)>,
    pub derived: Vec<Derived>,
    pub attrs: Vec<attribute>,
    pub span: span,
}

impl declarator {
    pub fn abstract_at(sp: span) -> declarator {
        declarator { name: None, derived: Vec::new(), attrs: Vec::new(), span: sp }
    }

    /// The parameters, if this declares a function.
    pub fn function_params(&self) -> Option<(&Vec<param_decl>, bool)> {
        match self.derived.last() {
            Some(DerivedFunction { params, variadic, .. }) => Some((params, *variadic)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct param_decl {
    pub spec: decl_spec,
    pub declarator: declarator,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct type_name {
    pub spec: decl_spec,
    pub declarator: declarator,
    pub span: span,
}

#[derive(Clone, Debug)]
pub enum Initializer {
    InitializerExpr(Box<expr>),
    InitializerList(Vec<(Vec<Designator>, Initializer)>, span),
}
pub use Initializer::*;

impl Initializer {
    pub fn span(&self) -> span {
        match self {
            InitializerExpr(e) => e.span,
            InitializerList(_, s) => *s,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Designator {
    DesignatorField(sym, span),
    DesignatorIndex(Box<expr>, span),
}
pub use Designator::*;

#[derive(Clone, Debug)]
pub struct init_declarator {
    pub declarator: declarator,
    pub init: Option<Initializer>,
    pub asm_label: Option<String>,
}

#[derive(Clone, Debug)]
pub struct declaration {
    pub spec: decl_spec,
    pub inits: Vec<init_declarator>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct function_def {
    pub spec: decl_spec,
    pub declarator: declarator,
    pub body: stmt,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct let_decl {
    pub mutable: bool,
    pub name: sym,
    pub name_span: span,
    pub ty: Option<Box<type_name>>,
    pub init: Option<Initializer>,
    pub attrs: Vec<attribute>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct static_assert {
    pub cond: Box<expr>,
    pub message: Option<String>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct import_name {
    pub name: sym,
    pub alias: Option<sym>,
    pub span: span,
}

#[derive(Clone, Debug)]
pub enum ImportKind {
    ImportKindNamed(Vec<import_name>),
    ImportKindNamespace(sym, span),
}
pub use ImportKind::*;

#[derive(Clone, Debug)]
pub struct import_decl {
    pub kind: ImportKind,
    pub from: String,
    pub from_span: span,
    pub span: span,
}

#[derive(Clone, Debug)]
pub struct export_list {
    pub names: Vec<import_name>,
    /// `export * from "m"`
    pub star: bool,
    pub from: Option<(String, span)>,
    pub span: span,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum UnOp {
    UnOpPlus,
    UnOpNeg,
    UnOpNot,
    UnOpBitNot,
    UnOpDeref,
    UnOpAddrOf,
    UnOpPreInc,
    UnOpPreDec,
    UnOpPostInc,
    UnOpPostDec,
}
pub use UnOp::*;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BinOp {
    BinOpMul,
    BinOpDiv,
    BinOpRem,
    BinOpAdd,
    BinOpSub,
    BinOpShl,
    BinOpShr,
    BinOpLt,
    BinOpGt,
    BinOpLe,
    BinOpGe,
    BinOpEq,
    BinOpNe,
    BinOpBitAnd,
    BinOpBitXor,
    BinOpBitOr,
    BinOpLogAnd,
    BinOpLogOr,
}
pub use BinOp::*;

impl BinOp {
    pub fn spelling(self) -> &'static str {
        match self {
            BinOpMul => "*",
            BinOpDiv => "/",
            BinOpRem => "%",
            BinOpAdd => "+",
            BinOpSub => "-",
            BinOpShl => "<<",
            BinOpShr => ">>",
            BinOpLt => "<",
            BinOpGt => ">",
            BinOpLe => "<=",
            BinOpGe => ">=",
            BinOpEq => "==",
            BinOpNe => "!=",
            BinOpBitAnd => "&",
            BinOpBitXor => "^",
            BinOpBitOr => "|",
            BinOpLogAnd => "&&",
            BinOpLogOr => "||",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(self, BinOpLt | BinOpGt | BinOpLe | BinOpGe | BinOpEq | BinOpNe)
    }
}

#[derive(Clone, Debug)]
pub struct expr {
    pub kind: ExprKind,
    pub span: span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    ExprKindIdent(sym),
    ExprKindQualified(sym, sym),
    ExprKindInt(sym),
    ExprKindFloat(sym),
    ExprKindChar(sym),
    ExprKindString(Vec<sym>),
    ExprKindBool(bool),
    ExprKindNullptr,
    ExprKindUnary(UnOp, Box<expr>),
    ExprKindBinary(BinOp, Box<expr>, Box<expr>),
    ExprKindAssign(Option<BinOp>, Box<expr>, Box<expr>),
    ExprKindCond(Box<expr>, Box<expr>, Box<expr>),
    ExprKindComma(Box<expr>, Box<expr>),
    ExprKindCall(Box<expr>, Vec<expr>),
    ExprKindIndex(Box<expr>, Box<expr>),
    ExprKindMember {
        base: Box<expr>,
        name: sym,
        name_span: span,
        arrow: bool,
    },
    ExprKindCast(Box<type_name>, Box<expr>),
    ExprKindCompound(Box<type_name>, Initializer),
    ExprKindSizeofExpr(Box<expr>),
    ExprKindSizeofType(Box<type_name>),
    ExprKindAlignofType(Box<type_name>),
    ExprKindAlignofExpr(Box<expr>),
    ExprKindGeneric(Box<expr>, Vec<(Option<type_name>, expr)>),
    /// `ref x` or `ref mut x`
    ExprKindRef(bool, Box<expr>),
    ExprKindVaArg(Box<expr>, Box<type_name>),
    ExprKindOffsetof(Box<type_name>, Vec<Designator>),
}
pub use ExprKind::*;

#[derive(Clone, Debug)]
pub struct stmt {
    pub kind: StmtKind,
    pub span: span,
}

#[derive(Clone, Debug)]
pub enum LabelKind {
    LabelKindNamed(sym),
    LabelKindCase(Box<expr>),
    LabelKindDefault,
}
pub use LabelKind::*;

#[derive(Clone, Debug)]
pub enum StmtKind {
    StmtKindCompound(Vec<BlockItem>),
    StmtKindExpr(Option<expr>),
    StmtKindIf(expr, Box<stmt>, Option<Box<stmt>>),
    StmtKindWhile(expr, Box<stmt>),
    StmtKindDoWhile(Box<stmt>, expr),
    StmtKindFor(Option<Box<BlockItem>>, Option<expr>, Option<expr>, Box<stmt>),
    StmtKindSwitch(expr, Box<stmt>),
    StmtKindLabeled(LabelKind, span, Box<stmt>),
    StmtKindBreak,
    StmtKindContinue,
    StmtKindReturn(Option<expr>),
    StmtKindGoto(sym),
    StmtKindUnsafe(Vec<BlockItem>),
}
pub use StmtKind::*;

#[derive(Clone, Debug)]
pub enum BlockItem {
    BlockItemDecl(declaration),
    BlockItemLet(let_decl),
    BlockItemStaticAssert(static_assert),
    BlockItemStmt(stmt),
}
pub use BlockItem::*;

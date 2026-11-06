/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
use std::cell::RefCell;
use std::collections::HashMap;

/// An interned string. Equal strings intern to equal symbols, so comparing
/// symbols is an integer comparison.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct sym(pub u32);

struct interner {
    map: HashMap<&'static str, u32>,
    strs: Vec<&'static str>,
}

thread_local! {
    static Interner: RefCell<interner> = RefCell::new(interner::with_predefined());
}

impl interner {
    fn with_predefined() -> interner {
        let mut it = interner { map: HashMap::new(), strs: Vec::new() };
        for s in Predefined {
            it.intern(s);
        }
        it
    }

    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.map.get(s) {
            return id;
        }
        // Symbols live for the whole compilation, so leaking keeps lookups cheap.
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
        let id = self.strs.len() as u32;
        self.strs.push(leaked);
        self.map.insert(leaked, id);
        id
    }
}

impl sym {
    pub fn intern(s: &str) -> sym {
        Interner.with(|it| sym(it.borrow_mut().intern(s)))
    }

    pub fn as_str(self) -> &'static str {
        Interner.with(|it| it.borrow().strs[self.0 as usize])
    }
}

impl std::fmt::Debug for sym {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl std::fmt::Display for sym {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

macro_rules! predefined_symbols {
    ($($name:ident = $text:expr,)*) => {
        const Predefined: &[&str] = &[$($text,)*];
        /// Index of each predefined symbol in the interner.
        #[repr(u32)]
        #[allow(dead_code)]
        enum Sym {
            $($name,)*
        }
        $(pub const $name: sym = sym(Sym::$name as u32);)*
    };
}

predefined_symbols! {
    SymEmpty = "",
    // Punctuators
    SymLParen = "(", SymRParen = ")", SymLBracket = "[", SymRBracket = "]",
    SymLBrace = "{", SymRBrace = "}", SymDot = ".", SymArrow = "->",
    SymPlusPlus = "++", SymMinusMinus = "--", SymAmp = "&", SymStar = "*",
    SymPlus = "+", SymMinus = "-", SymTilde = "~", SymBang = "!",
    SymSlash = "/", SymPercent = "%", SymShl = "<<", SymShr = ">>",
    SymLt = "<", SymGt = ">", SymLe = "<=", SymGe = ">=", SymEqEq = "==",
    SymNe = "!=", SymCaret = "^", SymPipe = "|", SymAmpAmp = "&&",
    SymPipePipe = "||", SymQuestion = "?", SymColon = ":", SymColonColon = "::",
    SymSemi = ";", SymEllipsis = "...", SymAssign = "=", SymStarEq = "*=",
    SymSlashEq = "/=", SymPercentEq = "%=", SymPlusEq = "+=", SymMinusEq = "-=",
    SymShlEq = "<<=", SymShrEq = ">>=", SymAmpEq = "&=", SymCaretEq = "^=",
    SymPipeEq = "|=", SymComma = ",", SymHash = "#", SymHashHash = "##",
    // C keywords
    SymAlignas = "alignas", SymAlignof = "alignof", SymAuto = "auto", SymBool = "bool",
    SymBreak = "break", SymCase = "case", SymChar = "char", SymConst = "const",
    SymConstexpr = "constexpr", SymContinue = "continue", SymDefault = "default",
    SymDo = "do", SymDouble = "double", SymElse = "else", SymEnum = "enum",
    SymExtern = "extern", SymFalse = "false", SymFloat = "float", SymFor = "for",
    SymGoto = "goto", SymIf = "if", SymInline = "inline", SymInt = "int",
    SymLong = "long", SymNullptr = "nullptr", SymRegister = "register",
    SymRestrict = "restrict", SymReturn = "return", SymShort = "short",
    SymSigned = "signed", SymSizeof = "sizeof", SymStatic = "static",
    SymStaticAssert = "static_assert", SymStruct = "struct", SymSwitch = "switch",
    SymThreadLocal = "thread_local", SymTrue = "true", SymTypedef = "typedef",
    SymTypeof = "typeof", SymTypeofUnqual = "typeof_unqual", SymUnion = "union",
    SymUnsigned = "unsigned", SymVoid = "void", SymVolatile = "volatile",
    SymWhile = "while", SymUAlignas = "_Alignas", SymUAlignof = "_Alignof",
    SymUAtomic = "_Atomic", SymUBitInt = "_BitInt", SymUBool = "_Bool",
    SymUComplex = "_Complex", SymUDecimal32 = "_Decimal32", SymUDecimal64 = "_Decimal64",
    SymUDecimal128 = "_Decimal128", SymUGeneric = "_Generic", SymUImaginary = "_Imaginary",
    SymUNoreturn = "_Noreturn", SymUStaticAssert = "_Static_assert",
    SymUThreadLocal = "_Thread_local",
    // GNU spellings that system headers use
    SymGnuAttribute = "__attribute__", SymGnuAttribute2 = "__attribute",
    SymGnuExtension = "__extension__", SymGnuAsm = "__asm__", SymGnuAsm2 = "__asm",
    SymAsm = "asm", SymGnuRestrict = "__restrict", SymGnuRestrict2 = "__restrict__",
    SymGnuInline = "__inline", SymGnuInline2 = "__inline__", SymGnuConst = "__const",
    SymGnuSigned = "__signed__", SymGnuVolatile = "__volatile__",
    SymGnuTypeof = "__typeof__", SymGnuTypeof2 = "__typeof", SymGnuAlignof = "__alignof__",
    SymInt128 = "__int128", SymInt128T = "__int128_t", SymUInt128T = "__uint128_t",
    SymBuiltinVaList = "__builtin_va_list", SymBuiltinVaArg = "__builtin_va_arg",
    SymBuiltinVaStart = "__builtin_va_start", SymBuiltinVaEnd = "__builtin_va_end",
    SymBuiltinVaCopy = "__builtin_va_copy", SymBuiltinOffsetof = "__builtin_offsetof",
    SymBuiltinUnreachable = "__builtin_unreachable", SymBuiltinTrap = "__builtin_trap",
    SymBuiltinExpect = "__builtin_expect",
    SymFunc = "__func__",
    // Crust keywords
    SymLet = "let", SymMut = "mut", SymUnsafe = "unsafe", SymModule = "module",
    SymExport = "export", SymImport = "import", SymRef = "ref", SymFrom = "from",
    SymAs = "as",
    // Attribute names
    SymCrust = "crust", SymDrop = "drop", SymSafe = "safe", SymCAbi = "c_abi",
    SymLen = "len", SymMain = "main", SymStatic2 = "'static",
    // Preprocessor
    SymDefine = "define", SymUndef = "undef", SymInclude = "include",
    SymIncludeNext = "include_next", SymIfdef = "ifdef", SymIfndef = "ifndef",
    SymElif = "elif", SymElifdef = "elifdef", SymElifndef = "elifndef", SymEndif = "endif",
    SymLine = "line", SymError = "error", SymWarning = "warning", SymPragma = "pragma",
    SymEmbed = "embed", SymOnce = "once", SymDefined = "defined",
    SymVaArgs = "__VA_ARGS__", SymVaOpt = "__VA_OPT__", SymUPragma = "_Pragma",
    SymHasInclude = "__has_include", SymHasIncludeNext = "__has_include_next",
    SymHasEmbed = "__has_embed", SymHasCAttribute = "__has_c_attribute",
    SymHasAttribute = "__has_attribute", SymHasBuiltin = "__has_builtin",
    SymHasFeature = "__has_feature", SymHasExtension = "__has_extension",
    SymFileMacro = "__FILE__", SymLineMacro = "__LINE__", SymCounterMacro = "__COUNTER__",
    SymDateMacro = "__DATE__", SymTimeMacro = "__TIME__", SymLimit = "limit",
    SymPrefix = "prefix", SymSuffix = "suffix", SymIfEmpty = "if_empty",
}

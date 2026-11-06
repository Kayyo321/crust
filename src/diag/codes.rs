/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/

/// Every diagnostic code, its one-line title, and its long explanation for
/// `crust --explain`. Codes are stable: once shipped, a code keeps its
/// meaning (docs/ARCHITECTURE.md, section 3).
pub struct code_info {
    pub code: &'static str,
    pub title: &'static str,
    pub explanation: &'static str,
}

pub fn lookup(code: &str) -> Option<&'static code_info> {
    Codes.iter().find(|c| c.code == code)
}

macro_rules! codes {
    ($($code:literal, $title:literal, $expl:literal;)*) => {
        pub static Codes: &[code_info] = &[$(code_info { code: $code, title: $title, explanation: $expl },)*];
    };
}

codes! {
    "CR1001", "#error directive", "An `#error` directive was reached. `#warning` reports with the same code as a warning.";
    "CR1002", "malformed preprocessing directive", "A preprocessing directive, `#if` expression, or `_Pragma` operator is malformed.";
    "CR1003", "unbalanced conditional directive", "`#if`, `#ifdef`, `#elif`, `#else`, and `#endif` must nest correctly within each file.";
    "CR1004", "bad macro invocation", "A function-like macro was called with the wrong number of arguments, or its argument list is not closed.";
    "CR1005", "include file not found", "The header named by `#include` or `#embed` was not found in the search path. Add a directory with `-I`.";
    "CR1006", "invalid token paste", "The `##` operator must produce a single valid preprocessing token.";
    "CR1101", "unterminated comment", "A `/*` comment has no closing `*/`.";
    "CR1102", "unterminated literal", "A string literal or character constant is not closed on the same line.";
    "CR1201", "syntax error", "The parser expected a different token here. The message names what it expected.";
    "CR1202", "invalid declaration specifiers", "The declaration specifiers conflict, repeat a storage class, or use one where it is not allowed.";
    "CR1203", "unknown type name", "A name used as a type is not a typedef in scope. C23 has no implicit `int` and no K&R parameter lists.";
    "CR1301", "unsupported feature", "The construct is valid C23 but is not supported by Crust 0.1. The message names the feature.";
    "CR1302", "undeclared identifier", "The name is not declared in this scope. Declare it, include the header that declares it, or import it from its module.";
    "CR1303", "conflicting declaration", "A name is declared twice in the same scope with incompatible types, or defined twice.";
    "CR1304", "incompatible types", "A value of one type is used where an incompatible type is required, for example in an assignment, an argument, or a return.";
    "CR1305", "invalid operands", "The operator cannot be applied to operands of these types.";
    "CR1306", "not assignable", "The left side of an assignment, or the operand of `++`, `--`, or `&`, must be a modifiable lvalue.";
    "CR1307", "wrong number of arguments", "A call passes more or fewer arguments than the function's prototype declares.";
    "CR1308", "not a constant expression", "This context needs a constant expression: an array size, a case label, an enumerator value, a bit-field width, a static initializer, or a `constexpr` or module-level initializer.";
    "CR1309", "no such member", "The struct or union has no member with this name.";
    "CR1310", "incomplete type", "The type is incomplete here. Its size is needed, so its definition must be visible.";
    "CR1311", "invalid initializer", "The initializer does not fit the object: too many elements, a designator that names no member, or a type mismatch.";
    "CR1312", "invalid jump or label", "A `goto` names a label that does not exist, a label is defined twice, or `break`, `continue`, `case`, or `default` appear outside the statement they belong to.";
    "CR1313", "invalid switch", "A `case` value is repeated, a `case` value is not an integer constant, or the controlling expression is not an integer.";
    "CR1314", "static assertion failed", "A `static_assert` condition is false.";
    "CR1315", "invalid use of a declaration", "The declaration is used in a way its kind does not allow, for example a typedef used as a value or a function declared with an array return type.";
    "CR1316", "called object is not a function", "Only functions and function pointers can be called.";
    "CR1317", "invalid return", "A `return` with a value in a `void` function, or a `return` without a value in a function that returns one.";
    "CR1318", "invalid cast", "The cast converts between types that cannot be converted, such as a struct to an integer.";
    "CR1319", "invalid use of void, array, or function type", "The expression has a type that cannot be used here, such as a `void` value or an array assigned as a whole.";
    "CR1320", "invalid attribute", "The attribute is unknown, has the wrong arguments, or is applied to something it does not apply to. Crust attributes are `[[crust::drop(f)]]`, `[[crust::safe]]`, and `[[crust::c_abi]]`.";
    "CR1321", "invalid borrow type", "Borrows (`ref T`, `ref mut T`) and slices (`ref [T]`) cannot be used here. In 0.1 they cannot be struct or union members, array elements, or global variables, and they exist only in Crust files.";
    "CR1322", "type annotation needed", "A `let` binding without an initializer needs a type, either written (`let x: int;`) or inferred from an assignment that comes before any use.";
    "CR1323", "qualifier discarded", "The conversion drops a `const` or `volatile` qualifier from the pointed-to type.";
    "CR1401", "module file included", "A `.crm` module cannot be included with `#include`. Import its exports with `import { ... } from \"name\";`.";
    "CR1402", "missing or misplaced module declaration", "A `.crm` file must begin with `module <name>;`, before any other declaration. Only comments may come first.";
    "CR1403", "module declaration outside a module", "Only `.crm` files are modules. A `.cr` or `.c` file cannot declare a module.";
    "CR1404", "unknown module", "No module with this name is part of the build, and no interface file for it was found. Same-package modules must be compiled first, or given as inputs in the same invocation; dependency packages are found through `--extern`.";
    "CR1405", "name not exported", "The imported module does not export this name. Only exported names can be imported. The message lists the module's exports.";
    "CR1406", "duplicate name", "An import introduces a name that is already imported or declared in this file. Rename one of them with `as`.";
    "CR1407", "import cycle", "Modules cannot import each other in a cycle in Crust 0.1 (D-031). Move the shared declarations into a third module.";
    "CR1408", "unsupported import or export form", "Crust 0.1 supports named imports, namespace imports, named exports, export lists, `export ... from`, and `export *` (D-033). Default exports, default imports, and side-effect imports are not supported.";
    "CR1409", "export outside a module", "Only `.crm` modules can export. A `.cr` file has no module name, and `.c` files are C.";
    "CR1410", "duplicate module", "Two files in the same build declare the same module name.";
    "CR1411", "unknown package", "The import names a package that was not given with `--extern`. With crepe, list the package under `[dependencies]` in `crepe.toml` (D-029).";
    "CR1412", "C ABI export uses a Crust-only type", "A `[[crust::c_abi]]` export may use only C-compatible types. Borrows, slices, and owned values cross into C only in their C-compatible form (docs/ABI.md, section 3).";
    "CR1413", "module-level initializer is not constant", "Every module-level variable in a `.crm` file takes a constant initializer, so importing a module runs no code (D-031).";
    "CR1414", "bad interface file", "A `.crmi` interface file is missing, corrupt, or from an incompatible compiler version. Rebuild the module.";
    "CR1415", "import in a C file", "`.c` and `.h` files are C. They cannot import Crust modules. Use the header that `crust --emit=header` generates.";
    "CR1501", "definition does not match header", "A function or variable defined in a `.cr` file has a different type from its declaration in the `.hr` header it includes (D-035). Make the signatures identical.";
    "CR1801", "safety checks disabled", "`--unchecked` turns off the safety analyses and `unsafe` gating for an input. It is a migration aid; the memory-safety guarantee does not cover code compiled this way. Runtime bounds checks stay on.";
    "CR0004", "non-exhaustive switch", "In checked code, a `switch` over an enum must have a `case` for every enumerator, or a `default` label.";
    "CR0106", "missing lifetime", "A function returns a borrow, but the lifetime of the result cannot be elided: there is not exactly one borrowed parameter. Name the lifetimes, for example `ref 'a int pick(ref 'a int a, ref 'a int b)`.";
    "CR0133", "unsafe operation outside unsafe", "This operation needs an `unsafe` block or function in Crust code: dereferencing an untracked raw pointer, pointer arithmetic, an unchecked cast, reading a union member, calling a C function or an `unsafe` function, or touching module-level mutable state. Add an `unsafe` block with a `SAFETY:` comment that says why the operation is sound.";
    "CR0381", "use of uninitialized value", "A variable is read before it is assigned on every path to the read (G-UNINIT).";
    "CR0382", "use of moved value", "An owned value was moved, and then used again (G-UAF, G-DF). After a move, the old binding cannot be used until it is assigned a new value.";
    "CR0384", "assignment to immutable binding", "A `let` binding without `mut` is assigned more than once. Declare it with `let mut` if it must change.";
    "CR0499", "two mutable borrows", "A value has a unique (`ref mut`) borrow that is still live, and a second unique borrow is taken (G-ALIAS).";
    "CR0502", "conflicting borrows", "A shared borrow and a unique borrow of the same value are live at the same time (G-ALIAS).";
    "CR0505", "move out of borrowed value", "A value is moved or dropped while a borrow of it is still live (G-UAF).";
    "CR0506", "assignment to borrowed value", "A value is assigned while a borrow of it is still live (G-ALIAS).";
    "CR0507", "move out of borrowed content", "An owned value cannot be moved out of a borrow, a pointer, an array element, or a global. Only a whole local variable, or a field of one, can be moved.";
    "CR0509", "move out of a type with a drop function", "A field cannot be moved out of a struct that has a `[[crust::drop]]` function, because the drop function expects the whole value.";
    "CR0515", "returns a reference to local data", "A function returns a borrow or a pointer to one of its own local variables. The local is gone when the function returns (G-UAF).";
    "CR0521", "borrowed data escapes", "The address of a local variable, or a borrow, is stored somewhere the checker cannot follow: a global, or memory reached through an untracked pointer. It could outlive the local (G-UAF).";
    "CR0594", "assignment through a shared borrow", "A shared borrow (`ref T`) is read-only. Use `ref mut` to mutate through a borrow.";
    "CR0596", "mutable borrow of immutable value", "`ref mut` needs a mutable place. Declare the binding with `let mut`.";
    "CR0597", "borrowed value does not live long enough", "A local variable goes out of scope while a borrow of it, or a pointer to it, is still used later (G-UAF).";
    "CR0623", "lifetime mismatch", "A borrow with one lifetime is used where a different lifetime is required, for example returning a parameter whose lifetime is not the return lifetime.";
    "CR0701", "index out of bounds", "A constant index is outside the bounds of the array (G-OOB).";
    "CR0702", "owned value copied", "An owned value (a type with a drop function, or one that contains such a type) cannot be copied implicitly, and cannot be a union member.";
    "CR1103", "stray character", "A character that is not part of any token survived preprocessing.";
}

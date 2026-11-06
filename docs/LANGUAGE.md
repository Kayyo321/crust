<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# The Crust language

Status: draft. Crust 0.1.1 implements this document except where a section says otherwise. Anything marked **(open)** is undecided. See [DECISIONS.md](DECISIONS.md).

## 1. What Crust is

Crust is C23 plus additions that let the compiler check memory safety. Every valid C23 translation unit is a valid Crust program in C mode. The additions are:

- `let` bindings, with immutability by default
- Borrows, written `ref x` and `ref mut x`, with lifetimes, and slices
- `unsafe` blocks and functions
- Modules (`.crm`): `module`, `export`, and `import`, the modern way to organize Crust projects. Source (`.cr`) with headers (`.hr`) stays for old-school code. See [MODULES.md](MODULES.md).
- Crust attributes, written as C23 vendor attributes: `[[crust::...]]`

C semantics still apply everywhere, with one difference. In checked code, the Crust rules add constraints. C code that is valid but unsafe is rejected unless the unsafe part is written inside `unsafe`.

## 2. Modes

File extension selects the default mode. Flags can override it. Crust source is `.cr`, Crust headers are `.hr`, and Crust modules are `.crm`. The reserved words `let`, `mut`, `unsafe`, `module`, `export`, `import`, and `ref` apply in all three. See [MODULES.md](MODULES.md), section 1.

| Mode | Default for | Syntax | Safety checks |
|---|---|---|---|
| Crust | `.cr`, `.hr`, `.crm` | C23 + Crust additions. `.crm` files are modules ([MODULES.md](MODULES.md)). | On |
| C | `.c`, `.h` | C23 only; `let`, `mut`, `unsafe`, `module`, `export`, `import`, `ref` are identifiers | On, without `unsafe` gating (D-037) |
| Unchecked | nothing; `--unchecked` only | either | Off. Migration aid. Emits a warning on every use. |

`--std=crust` and `--std=c23` override the syntax mode. Checks stay on unless `--unchecked` is given. See [CLI.md](CLI.md).

Why `.c` files are checked by default: the goal is that C code compiled by Crust gets the guarantees. A C file that opts out must say so.

C files cannot spell `unsafe`, so C mode has no `unsafe` gating (D-037): calling C functions, dereferencing pointers of unknown origin, and pointer arithmetic are allowed there, and those operations are outside the guarantee. Initialization, moves, drops, borrows of tracked pointers, exhaustive `switch`, and bounds checks on arrays still apply.

## 3. Declarations: `let`

`let` introduces a binding. Bindings are immutable unless marked `mut`. The type is inferred from the initializer unless written.

```c
let count = 0;            // immutable, type int
let mut total = 0;        // mutable
total += count;           // ok
// count = 1;             // error: `count` is immutable
let name: const char* = "crust";   // explicit type
let later;                // type from the first assignment
later = 2.5;              // `later` is a double from here on
```

Plain C declarations are unchanged and mutable, and they are still checked:

```c
int legacy = 3;           // C declaration: mutable, checked
```

Rationale: immutable-by-default makes aliasing easier to check, which is why Rust does it.

## 4. Ownership and moves

Some values have a single owner. Examples: memory from the Crust allocator, file handles, and any type that declares a destructor. Assigning or passing an owned value moves it. The source cannot be used afterward.

```c
let buf = owned_bytes(64);    // owned (illustrative allocator)
let other = buf;              // moved
// use(buf);                  // error: `buf` was moved into `other`
```

Scalars, pointers, and structs with no owned fields are copied, as in C.

## 5. Borrows and lifetimes

A borrow is a reference that does not own its target. Borrows use their own spelling in every mode (D-018):

| Form | Meaning |
|---|---|
| `ref x` | Shared borrow of `x`: read-only |
| `ref mut x` | Unique borrow of `x`: can mutate |
| `ref T`, `ref 'a T` | Type of a shared borrow of a `T`, with an optional lifetime |
| `ref mut T`, `ref 'a mut T` | Type of a unique borrow |
| `*r`, `r->field` | The borrowed value |

Rules:

1. A value can have any number of shared borrows, or exactly one unique borrow, at a time.
2. A borrow cannot outlive the value it borrows.
3. `ref mut` needs a mutable place: a `let mut` binding, a plain C variable, or data behind another `ref mut`.
4. A borrow ends at its last use, not at the end of its scope.
5. Lifetimes are elided in simple cases. A function that returns a borrow takes the lifetime of its only borrowed parameter. Write `'a` only where that fails.

```c
void bump(ref mut int n) {
    *n += 1;
}

ref 'a int pick(ref 'a int a, ref 'a int b, bool first) {
    return first ? a : b;
}

int main(void) {
    let mut count = 41;
    bump(ref mut count);           // unique borrow, ends after the call
    let view;
    {
        let owner = 5;
        view = ref owner;          // shared borrow of `owner`
    }                              // error CR0597: `owner` does not live long enough
    return *view;
}
```

`&x` is always C address-of. It gives a raw pointer, which the checker tracks when it points at a live local (see [MEMORY_SAFETY.md](MEMORY_SAFETY.md#5-raw-pointers)). A borrow converts to a raw pointer only with an explicit cast, and a shared borrow converts only to a pointer to `const`.

In 0.1, a borrow cannot be a struct or union member, an array element, or a global variable (D-041). Lifetime parameters on structs are open (D-048).

## 6. Arrays, slices, and bounds

- An array `T a[N]` knows its length. Constant indexes are checked at compile time (CR0701). Variable indexes are checked at run time and trap on failure.
- A slice is a pointer plus a length: `ref [T]` (shared) or `ref mut [T]` (unique). `s.len` is its length, as a `size_t`, and `s[i]` is bounds-checked. A borrow of an array converts to a slice: `int a[4]; ref [int] s = ref a;`.
- At the C boundary a slice is a struct of a `T *` and a `size_t`, in that order (see [ABI.md](ABI.md)).
- A bare `T *` with a separate length parameter is accepted. Indexing it is pointer arithmetic, so it needs `unsafe` in Crust code. Slices are the checked alternative.

## 7. Initialization and control flow

- Reading a variable before it is assigned on every path is an error.
- In checked code, `switch` over an enum must name every enumerator or include `default`.
- Null: a raw pointer can be null. A borrow (`ref T`) cannot. Nullable checked pointers are **(open, D-023)**.
- A non-`void` Crust function must return a value on every path (CR0381). In C mode, and in `main`, falling off the end returns zero.

## 8. Drop

An owned value is released when its scope ends, unless it was moved. A type that owns a resource declares how to release it:

```c
// illustrative
[[crust::drop(vec_free)]]
struct vec {
    int *data;
    size_t len;
};
```

The drop function has type `void f(struct vec *)` or `void f(ref mut struct vec)`, and is declared at file scope. A struct with a drop function, or with an owned member, is owned. After the drop function runs, the owned members are dropped in turn (D-040).

The compiler inserts the drop on every exit path, including early `return`, `break`, and `goto`. When a value is moved on some paths only, a hidden drop flag decides at run time. Assigning to an owned variable drops its old value first.

An owned value cannot be a union member, cannot be built field by field, and cannot be moved out of a pointer, an array element, a global, or a field of a type with a drop function.

## 9. Unsafe

`unsafe` marks code the programmer has to prove correct. It is required for:

- Dereferencing a raw pointer the checker cannot track (see MEMORY_SAFETY.md)
- Unchecked indexing and pointer arithmetic
- Casts between unrelated types, and reads of inactive union members
- Calling `malloc` or `free` directly. Checked code gets memory from the Crust allocator, which returns owned values.
- Calling a C function. C declarations are unsafe to call by default.
- Calling a function declared `unsafe`
- Calling through a function pointer
- Reading a union member
- Indexing an array of unknown length, such as a flexible array member
- Reading or writing a module-level `let mut`

```c
unsafe {
    // SAFETY: `p` was returned by `vec_reserve` and has `n` valid elements.
    p[n - 1] = 0;
}
```

A declaration can assert that a C function is safe with `[[crust::safe]]`, for example by redeclaring it after its header: `[[crust::safe]] int puts(const char *s);`. The assertion is a claim the compiler cannot check. It is reviewed like any `unsafe` block.

A function is unsafe to call when it is declared `unsafe int f(...)`. A function is a C function when it is declared in a C header or a `.c` file, or when a module declares it without defining it.

## 10. Attributes and extension points

Crust-specific attributes use the C23 vendor-attribute form `[[crust::name]]`. Attributes are preferred over keywords because they cannot break C code. The planned attributes are `drop`, `safe`, and `c_abi` (see [MODULES.md](MODULES.md), section 10). Each one is defined in this document before it is used.

## 11. Undefined behavior

In checked code, C undefined behavior that can corrupt memory is turned into a trap. Examples: out-of-range indexes, and signed overflow in index computations. Which kinds of undefined behavior are covered is **(open, D-021)**. Behavior that is undefined in C and out of scope for the guarantee stays documented as undefined.

Until D-022 is decided, 0.1 gives integer arithmetic defined results (D-042): it wraps, shift counts are taken modulo the operand width, signed division overflow wraps, integer division by zero traps, and float-to-integer conversion saturates.

## 12. Not in v1, or not yet

Not supported by 0.1.0: variable-length arrays, `_Complex`, `_BitInt`, decimal floating types, `_Atomic` semantics (it is accepted and ignored), `va_arg` of a struct passed in registers, and targets other than x86-64 Linux (D-044).

Not in v1:


- Generics. Deferred; see D-026.
- Closures, async, and exceptions. Not planned.
- Threads and `Send`/`Sync`. Planned after M7.
- Default exports and dynamic imports. See [MODULES.md](MODULES.md), section 11.

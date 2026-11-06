<!--
/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
-->

# The Crust language

Status: draft. Examples are illustrative. Anything marked **(open)** is undecided. See [DECISIONS.md](DECISIONS.md).

## 1. What Crust is

Crust is C23 plus additions that let the compiler check memory safety. Every valid C23 translation unit is a valid Crust program in C mode. The additions are:

- `let` bindings, with immutability by default
- Lifetimes on borrowed references
- `unsafe` blocks and functions
- Crust attributes, written as C23 vendor attributes: `[[crust::...]]`

C semantics still apply everywhere, with one difference. In checked code, the Crust rules add constraints. C code that is valid but unsafe is rejected unless the unsafe part is written inside `unsafe`.

## 2. Modes

File extension selects the default mode. Flags can override it.

| Mode | Default for | Syntax | Safety checks |
|---|---|---|---|
| Crust | `.cr` | C23 + Crust additions | On |
| C | `.c`, `.h` | C23 only; `let`, `mut`, `unsafe`, `import` are identifiers | On |
| Unchecked | nothing; `--unchecked` only | either | Off. Migration aid. Emits a warning on every use. |

`--std=crust` and `--std=c23` override the syntax mode. Checks stay on unless `--unchecked` is given. See [CLI.md](CLI.md).

Why `.c` files are checked by default: the goal is that C code compiled by Crust gets the guarantees. A C file that opts out must say so.

## 3. Declarations: `let`

`let` introduces a binding. Bindings are immutable unless marked `mut`. The type is inferred from the initializer unless written.

```c
let count = 0;            // immutable, type int
let mut total = 0;        // mutable
total += count;           // ok
// count = 1;             // error: `count` is immutable
let name: const char* = "crust";   // explicit type
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

A borrow is a reference that does not own its target. Two kinds:

- `&'a T`: shared, read-only
- `&'a mut T`: unique, can mutate

Rules:

1. A value can have any number of shared borrows, or exactly one unique borrow, at a time.
2. A borrow cannot outlive the value it borrows.
3. Lifetimes are elided in simple cases. Write `'a` only where elision fails.

```c
// illustrative
let view;
{
    let owner = owned_bytes(64);
    view = &owner;                 // shared borrow of `owner`
}                                  // `owner` is released here
// use(view);                      // error: `view` outlives `owner`
```

The surface syntax for borrows is **(open, D-018)**. In C's address-of, `&x` gives a raw pointer. In `.cr` checked code it could give a borrow. Raw pointers in checked code are covered in [MEMORY_SAFETY.md](MEMORY_SAFETY.md#5-raw-pointers).

## 6. Arrays, slices, and bounds

- An array `T a[N]` knows its length. Constant indexes are checked at compile time. Variable indexes are checked at run time and trap on failure, unless the compiler proves them in range.
- A slice is a pointer plus a length. Its type is **(open, D-018)**. Indexing a slice is bounds-checked.
- A bare `T *` with a separate length parameter is accepted in C mode. Indexing it is unchecked, so it needs `unsafe`. Slices are the checked alternative.

## 7. Initialization and control flow

- Reading a variable before it is assigned on every path is an error.
- In checked code, `switch` over an enum must name every enumerator or include `default`.
- Null: a raw pointer can be null. A borrow (`&'a T`) cannot. Nullable checked pointers are **(open, D-023)**.

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

The compiler inserts the drop on every exit path, including early `return` and `goto`.

## 9. Unsafe

`unsafe` marks code the programmer has to prove correct. It is required for:

- Dereferencing a raw pointer the checker cannot track (see MEMORY_SAFETY.md)
- Unchecked indexing and pointer arithmetic
- Casts between unrelated types, and reads of inactive union members
- Calling `malloc` or `free` directly. Checked code gets memory from the Crust allocator, which returns owned values.
- Calling a C function. C declarations are unsafe to call by default.
- Calling a function declared `unsafe`

```c
unsafe {
    // SAFETY: `p` was returned by `vec_reserve` and has `n` valid elements.
    p[n - 1] = 0;
}
```

A declaration can assert that a C function is safe with `[[crust::safe]]`. The assertion is a claim the compiler cannot check. It is reviewed like any `unsafe` block.

## 10. Attributes and extension points

Crust-specific attributes use the C23 vendor-attribute form `[[crust::name]]`. Attributes are preferred over keywords because they cannot break C code. The planned attributes are `drop`, `safe`, and `export`. Each one is defined in this document before it is used.

## 11. Undefined behavior

In checked code, C undefined behavior that can corrupt memory is turned into a trap. Examples: out-of-range indexes, and signed overflow in index computations. Which kinds of undefined behavior are covered is **(open, D-021)**. Behavior that is undefined in C and out of scope for the guarantee stays documented as undefined.

## 12. Not in v1

- Generics. Deferred; see D-026.
- Closures, async, and exceptions. Not planned.
- Threads and `Send`/`Sync`. Planned after M7.
- Module system. The import syntax is **(open, D-020)**.

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Borrows: `ref x` and `ref mut x` (LANGUAGE.md, section 5; D-018).
//
//   crust borrows.cr -o borrows && ./borrows

#include <stdio.h>

[[crust::safe]] int printf(const char *format, ...);

struct counter {
    int hits;
    int misses;
};

// A shared borrow: read-only, never null.
int total(ref struct counter c) {
    return c->hits + c->misses;
}

// A unique borrow: the only way to reach `c` while it lives.
void record(ref mut struct counter c, bool hit) {
    if (hit) {
        c->hits += 1;
    } else {
        c->misses += 1;
    }
}

void bump(ref mut int n) {
    *n += 1;
}

int main(void) {
    let mut c = (struct counter){ .hits = 0, .misses = 0 };
    record(ref mut c, true);
    record(ref mut c, false);
    record(ref mut c, true);

    // Many shared borrows can be live at once.
    let a = ref c;
    let b = ref c;
    printf("total=%d hits=%d\n", total(a), b->hits);

    // A borrow ends at its last use (non-lexical lifetimes), so a unique
    // borrow is fine afterwards.
    let r = ref mut c.misses;
    *r += 10;
    printf("misses=%d\n", c.misses);

    // Reborrow through a unique borrow.
    let mut n = 41;
    let outer = ref mut n;
    bump(ref mut *outer);
    printf("n=%d\n", n);

    // `&x` is still C address-of: a raw pointer, tracked because it
    // points at a live local, so dereferencing it needs no `unsafe`.
    int x = 5;
    int *p = &x;
    *p = 6;
    printf("x=%d\n", x);
    return 0;
}

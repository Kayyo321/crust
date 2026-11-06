/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// `unsafe` and `[[crust::safe]]` (LANGUAGE.md, section 9). Everything
// that the checker cannot prove sits inside `unsafe`, with a SAFETY comment
// that says why it holds.
//
//   crust unsafe.cr -o unsafe && ./unsafe

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Promise that a C function is safe to call. Checked by review, not by
// the compiler.
[[crust::safe]] int printf(const char *format, ...);
[[crust::safe]] size_t strlen(const char *s);

// An unsafe function: its callers take on its preconditions.
// Precondition: `p` points to at least `n` ints.
unsafe int sum_raw(const int *p, size_t n) {
    int s = 0;
    for (size_t i = 0; i < n; i++) {
        s += p[i];                  // pointer arithmetic: fine inside
    }
    return s;
}

union bits {
    float f;
    unsigned u;
};

int add(int a, int b) {
    return a + b;
}

// Module-level mutable state needs `unsafe` for every use.
let mut calls = 0;

int main(void) {
    // A pointer to a live local is tracked: no `unsafe` needed.
    int local = 10;
    int *tracked = &local;
    *tracked += 1;

    // malloc is a C function, and its result is untracked.
    int *heap;
    unsafe {
        // SAFETY: malloc is called with a non-zero size; the null check
        // below runs before any use.
        heap = malloc(3 * sizeof(int));
    }
    if (heap == nullptr) {
        return 1;
    }
    unsafe {
        // SAFETY: heap holds 3 ints, all written before sum_raw reads them,
        // and it is freed exactly once.
        heap[0] = 1;
        heap[1] = 2;
        heap[2] = local;
        printf("sum=%d\n", sum_raw(heap, 3));
        free(heap);
    }

    // Reading a union member is unsafe: the checker cannot know which
    // member is active.
    union bits b;
    b.f = 1.0f;
    unsigned raw;
    unsafe {
        // SAFETY: `f` was written last, and every bit pattern is a valid
        // unsigned int.
        raw = b.u;
    }
    printf("bits=0x%08x len=%zu\n", raw, strlen("crust"));

    // Calls through function pointers are unsafe.
    int (*op)(int, int) = add;
    unsafe {
        // SAFETY: `op` points to `add`, a safe function.
        calls += 1;
        printf("op=%d calls=%d\n", op(2, 3), calls);
    }
    return 0;
}

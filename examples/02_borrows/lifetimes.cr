/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Lifetimes on borrows (LANGUAGE.md, section 5; D-005).
//
//   crust lifetimes.cr -o lifetimes && ./lifetimes

#include <stdio.h>

[[crust::safe]] int printf(const char *format, ...);

struct pair {
    int left;
    int right;
};

// One borrowed parameter: the result takes its lifetime (elision).
ref int left_of(ref struct pair p) {
    return ref p->left;
}

// Two borrowed parameters: name the lifetime the result shares.
ref 'a int larger(ref 'a int x, ref 'a int y) {
    return *x >= *y ? x : y;
}

// Different lifetimes are fine when the result uses only one of them.
ref 'a int first(ref 'a int x, ref 'b int y) {
    (void)y;
    return x;
}

int main(void) {
    let p = (struct pair){ .left = 3, .right = 9 };
    let l = left_of(ref p);
    let a = 4;
    let b = 8;
    let big = larger(ref a, ref b);
    let f = first(ref a, ref b);
    printf("left=%d larger=%d first=%d\n", *l, *big, *f);
    return 0;
}

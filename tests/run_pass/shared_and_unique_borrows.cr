/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

struct counter {
    int hits;
    int misses;
};

void record(ref mut struct counter c, bool hit) {
    if (hit) {
        c->hits += 1;
    } else {
        c->misses += 1;
    }
}

int rate(ref struct counter c) {
    let all = c->hits + c->misses;
    return all == 0 ? 0 : c->hits * 100 / all;
}

void swap(ref mut int a, ref mut int b) {
    let t = *a;
    *a = *b;
    *b = t;
}

ref 'a int larger(ref 'a int a, ref 'a int b) {
    return *a >= *b ? a : b;
}

int main(void) {
    let mut c = (struct counter){ .hits = 0, .misses = 0 };
    record(ref mut c, true);
    record(ref mut c, true);
    record(ref mut c, false);
    record(ref mut c, true);
    let mut x = 1;
    let mut y = 2;
    swap(ref mut x, ref mut y);
    let first = ref x;
    let second = ref x;
    let big = larger(first, ref y);
    unsafe {
        // SAFETY: the format specifiers match the int arguments.
        printf("%d %d %d %d %d %d %d\n", c.hits, c.misses, rate(ref c), x, y, *first + *second, *big);
    }
    let r = ref mut x;
    *r = 40;
    let r2 = ref mut x;
    *r2 += 2;
    unsafe {
        // SAFETY: the format specifier matches the int argument.
        printf("%d\n", x);
    }
    return 0;
}

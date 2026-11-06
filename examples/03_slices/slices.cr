/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Slices: `ref [T]` and `ref mut [T]` (LANGUAGE.md, section 6; D-018),
// and bounds checks (G-OOB).
//
//   crust slices.cr -o slices && ./slices
//   ./slices oops      # an out-of-bounds index traps at run time

#include <stdio.h>

[[crust::safe]] int printf(const char *format, ...);

long sum(ref [int] xs) {
    let mut s = 0L;
    for (size_t i = 0; i < xs.len; i++) {
        s += xs[i];                 // checked against xs.len
    }
    return s;
}

void double_all(ref mut [int] xs) {
    for (size_t i = 0; i < xs.len; i++) {
        xs[i] *= 2;
    }
}

int main(int argc, char **argv) {
    (void)argv;
    int data[5] = { 1, 2, 3, 4, 5 };
    printf("sum=%ld\n", sum(ref data));     // an array borrow becomes a slice
    double_all(ref mut data);
    let view: ref [int] = ref data;
    printf("sum=%ld len=%zu\n", sum(view), view.len);

    // data[5] = 0;                // error CR0701: constant index out of bounds
    int i = argc > 1 ? 7 : 4;      // a runtime index is checked at run time
    data[i] = 0;                   // with an argument: "index out of bounds", abort
    printf("last=%d\n", data[4]);
    return 0;
}

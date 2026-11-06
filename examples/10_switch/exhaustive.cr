/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Exhaustive `switch` over enums (MEMORY_SAFETY.md, section 3). In checked
// code, a switch over an enum names every enumerator or has `default`.
// Leaving one out is error CR0004.
//
//   crust exhaustive.cr -o exhaustive && ./exhaustive

#include <stdio.h>

[[crust::safe]] int printf(const char *format, ...);

enum Shape : unsigned char {
    ShapeCircle,
    ShapeSquare,
    ShapeTriangle,
};

int corners(enum Shape s) {
    switch (s) {
    case ShapeCircle:
        return 0;
    case ShapeSquare:
        return 4;
    case ShapeTriangle:
        return 3;
    }
    return -1;
}

const char *name(enum Shape s) {
    switch (s) {
    case ShapeCircle:
        return "circle";
    default:                        // `default` also covers the rest
        return "polygon";
    }
}

int main(void) {
    for (int i = ShapeCircle; i <= ShapeTriangle; i++) {
        let s = (enum Shape)i;
        printf("%s %d\n", name(s), corners(s));
    }
    return 0;
}

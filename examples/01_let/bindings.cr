/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// `let` bindings (LANGUAGE.md, section 3; D-004, D-011).
//
//   crust bindings.cr -o bindings && ./bindings

#include <stdio.h>

[[crust::safe]] int printf(const char *format, ...);

int main(void) {
    let count = 3;                  // immutable, type inferred: int
    let mut total = 0;              // mutable
    for (int i = 1; i <= count; i++) {
        total += i;
    }

    let ratio: double = 2.5;        // explicit type after a colon
    let name: const char * = "crust";

    let later;                      // no type yet: it comes from the
    if (total > 5) {                // first assignment on any path
        later = 10;
    } else {
        later = 20;
    }

    int legacy = 7;                 // plain C declarations stay mutable
    legacy += 1;

    // count = 4;                   // error CR0384: `count` is immutable
    // total = later + ;            // (typos are still C syntax errors)

    printf("count=%d total=%d ratio=%.1f name=%s later=%d legacy=%d\n",
           count, total, ratio, name, later, legacy);
    return 0;
}

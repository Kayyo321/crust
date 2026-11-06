/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Operations that need `unsafe` in Crust files (CR0133). Wrap each in
// `unsafe { ... }` with a `// SAFETY:` comment that says why it holds.

#include <stdlib.h>

union word {
    int i;
    float f;
};

int main(void) {
    int *p = malloc(4);             //~ ERROR CR0133
    int arr[4] = { 0 };
    int *q = arr + 1;               //~ ERROR CR0133
    union word w;
    w.i = 1;
    float f = w.f;                  //~ ERROR CR0133
    (void)p;
    (void)q;
    return (int)f;
}

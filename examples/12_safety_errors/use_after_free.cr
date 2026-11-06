/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// G-UAF and G-DF: what the checker rejects. The comment at the end of a
// flagged line names its diagnostic; `crust --explain CRnnnn` explains it.
//
//   crust use_after_free.cr -o x      # fails with the errors below

#include <stdlib.h>

[[crust::drop(box_free)]]
struct box {
    int *value;
};

void box_free(ref mut struct box b) {
    unsafe {
        // SAFETY: `value` is owned by the box and freed only here.
        free(b->value);
    }
}

struct box box_new(void);
void consume(struct box b);

int *saved;

int *dangling(void) {
    int local = 1;
    return &local;                  //~ ERROR CR0515
}

int main(void) {
    let a = box_new();
    consume(a);
    consume(a);                     //~ ERROR CR0382

    let b = box_new();
    let r = ref b;
    let moved = b;                  //~ ERROR CR0505
    (void)r->value;
    consume(moved);

    ref int view;
    {
        let short_lived = 5;
        view = ref short_lived;
    }                               //~ ERROR CR0597
    (void)*view;

    int x = 3;
    saved = &x;                     //~ ERROR CR0521
    return 0;
}

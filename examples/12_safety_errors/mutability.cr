/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Mutation needs `mut`, and a shared borrow is read-only. These are found
// during type checking, before the CrIR analyses run.

void write(ref mut int r) {
    *r = 1;
}

int main(void) {
    let frozen = 7;
    write(ref mut frozen);          //~ ERROR CR0596

    let mut x = 0;
    let view = ref x;
    *view = 2;                      //~ ERROR CR0594
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// G-ALIAS: many shared borrows, or one unique borrow, never both. A borrow
// lasts until its last use.

void read(ref int r) {
    (void)*r;
}

void write(ref mut int r) {
    *r = 1;
}

int main(void) {
    let mut x = 0;

    let shared = ref x;
    x = 1;                          //~ ERROR CR0506
    read(shared);

    let unique = ref mut x;
    let other = ref x;              //~ ERROR CR0502
    write(unique);
    read(other);

    let first = ref mut x;
    let second = ref mut x;         //~ ERROR CR0499
    write(first);
    write(second);
    return 0;
}

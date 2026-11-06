/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-ALIAS
void add_both(ref mut int a, ref mut int b) {
    *a += 1;
    *b += 1;
}

int main(void) {
    let mut x = 1;
    add_both(ref mut x, ref mut x);   //~ ERROR CR0499
    return x;
}

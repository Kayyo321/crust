/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
void set(ref int p) {
    *p = 2;   //~ ERROR CR0594
}

int main(void) {
    let mut x = 1;
    set(ref x);
    return x;
}

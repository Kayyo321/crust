/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-ALIAS
int main(void) {
    let mut x = 1;
    let view = ref x;
    int *p = &x;   //~ ERROR CR0502
    *p = 2;
    return *view;
}

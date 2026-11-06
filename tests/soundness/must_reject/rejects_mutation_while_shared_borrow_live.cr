/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-ALIAS
int main(void) {
    let mut x = 1;
    let view = ref x;
    x += 1;   //~ ERROR CR0506
    return *view;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    let mut x = 1;
    let view = ref x;
    x = 2;   //~ ERROR CR0506
    return *view;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    let x = 1;
    let r = ref mut x;   //~ ERROR CR0596
    *r = 2;
    return x;
}

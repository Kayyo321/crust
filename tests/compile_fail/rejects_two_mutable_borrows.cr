/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    let mut x = 1;
    let a = ref mut x;
    let b = ref mut x;   //~ ERROR CR0499
    *a += 1;
    *b += 1;
    return x;
}

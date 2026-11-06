/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    let mut x = 1;
    let shared = ref x;
    let unique = ref mut x;   //~ ERROR CR0502
    *unique = 2;
    return *shared;
}

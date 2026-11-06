/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-ALIAS
int main(void) {
    let mut x = 1;
    let shared = ref x;
    let copy = *shared;
    let unique = ref mut x;
    *unique += copy;
    let again = ref x;
    return *again - 2;
}

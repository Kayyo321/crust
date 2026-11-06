/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-ALIAS
int main(void) {
    let mut x = 1;
    {
        let view = ref x;
        if (*view != 1) {
            return 1;
        }
    }
    x = 2;
    let unique = ref mut x;
    *unique *= 3;
    return x - 6;
}

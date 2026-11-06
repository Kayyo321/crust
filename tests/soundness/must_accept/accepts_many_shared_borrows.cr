/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-ALIAS
int sum3(ref int a, ref int b, ref int c) {
    return *a + *b + *c;
}

int main(void) {
    let x = 4;
    let a = ref x;
    let b = ref x;
    return sum3(a, b, ref x) - 12;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int zero(void) {
    return 0;
}

int main(void) {
    int (*f)(void) = zero;
    return f();   //~ ERROR CR0133
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    const int x = 1;
    const int *p = &x;
    int *q = p;   //~ ERROR CR1323
    return *q;
}

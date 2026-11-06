/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    int a[4] = {1, 2, 3, 4};
    int *p = &a[0];
    int *q = p + 1;   //~ ERROR CR0133
    (void)q;
    return 0;
}

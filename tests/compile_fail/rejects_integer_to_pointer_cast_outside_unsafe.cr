/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    int *p = (int *)4096;   //~ ERROR CR0133
    (void)p;
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
int main(void) {
    int *p;
    {
        int x = 1;
        p = &x;
    }   //~ ERROR CR0597
    return *p;
}

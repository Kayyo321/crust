/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-OOB
int main(void) {
    int a[3] = {1, 2, 3};
    a[3] = 4;   //~ ERROR CR0701
    return a[0];
}

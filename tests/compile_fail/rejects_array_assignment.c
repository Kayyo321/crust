/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    int a[2] = {1, 2};
    int b[2] = {3, 4};
    a = b;   //~ ERROR CR1319
    return a[0];
}

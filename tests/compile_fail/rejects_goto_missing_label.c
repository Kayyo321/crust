/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    goto nowhere;   //~ ERROR CR1312
    return 0;
}

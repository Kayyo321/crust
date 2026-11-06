/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    int k = 2;
    switch (k) {
    case k:   //~ ERROR CR1308
        return 1;
    default:
        return 0;
    }
}

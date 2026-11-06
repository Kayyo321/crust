/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    int k = 1;
    switch (k) {
    case 1:
        return 1;
    case 1:   //~ ERROR CR1313
        return 2;
    default:
        return 0;
    }
}

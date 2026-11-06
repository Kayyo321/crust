/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#define Add(a, b) ((a) + (b))
int main(void) {
    return Add(1);   //~ ERROR CR1004
}

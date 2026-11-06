/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#define Paste(a, b) a ## b
int main(void) {
    int x = Paste(+, -) 1;   //~ ERROR CR1006
    return x;
}

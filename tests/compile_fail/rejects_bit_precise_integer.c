/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(void) {
    _BitInt(8) small = 0;   //~ ERROR CR1301
    return (int)small;
}

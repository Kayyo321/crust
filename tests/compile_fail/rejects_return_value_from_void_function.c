/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
void nothing(void) {
    return 1;   //~ ERROR CR1317
}
int main(void) {
    nothing();
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int add(int a, int b) {
    return a + b;
}
int main(void) {
    return add(1);   //~ ERROR CR1307
}

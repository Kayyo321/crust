/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
[[crust::frobnicate]] int answer(void) {   //~ ERROR CR1320
    return 42;
}
int main(void) {
    return answer() - 42;
}

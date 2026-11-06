/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
export int answer(void) {   //~ ERROR CR1409
    return 42;
}
int main(void) {
    return answer() - 42;
}

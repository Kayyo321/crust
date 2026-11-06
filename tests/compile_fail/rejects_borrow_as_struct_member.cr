/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
struct holder {
    ref int target;   //~ ERROR CR1321
};
int main(void) {
    return 0;
}

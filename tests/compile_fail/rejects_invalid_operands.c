/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
struct point {
    int x;
};
int main(void) {
    struct point a = {1};
    struct point b = {2};
    return a + b;   //~ ERROR CR1305
}

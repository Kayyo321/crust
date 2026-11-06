/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
struct point {
    int x;
    int y;
};
int main(void) {
    struct point p = {1, 2};
    int n = p;   //~ ERROR CR1304
    return n;
}

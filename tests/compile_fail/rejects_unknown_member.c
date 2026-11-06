/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
struct point {
    int x;
    int y;
};
int main(void) {
    struct point p = {1, 2};
    return p.z;   //~ ERROR CR1309
}

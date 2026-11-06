/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UNINIT
struct point {
    int x;
    int y;
};

int main(void) {
    struct point p;
    p.x = 1;
    return p.y;   //~ ERROR CR0381
}

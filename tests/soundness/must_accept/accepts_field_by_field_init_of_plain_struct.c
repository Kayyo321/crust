/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UNINIT
struct point {
    int x;
    int y;
    int z;
};

int main(void) {
    struct point p;
    p.x = 1;
    p.y = 2;
    p.z = 3;
    struct point q = p;
    return q.x + q.y + q.z - 6;
}

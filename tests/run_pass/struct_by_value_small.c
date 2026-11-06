/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

struct point {
    int x;
    int y;
};

struct point make_point(int x, int y) {
    struct point p = {x, y};
    return p;
}

struct point add_points(struct point a, struct point b) {
    return make_point(a.x + b.x, a.y + b.y);
}

struct tiny {
    char c;
    short s;
};

struct tiny bump_tiny(struct tiny t) {
    t.c += 1;
    t.s *= 2;
    return t;
}

int main(void) {
    struct point a = make_point(1, 2);
    struct point b = make_point(-10, 20);
    struct point c = add_points(a, b);
    printf("%d %d\n", c.x, c.y);
    struct tiny t = bump_tiny((struct tiny){'a', 300});
    printf("%c %d\n", t.c, t.s);
    printf("%d\n", a.x);
    return 0;
}

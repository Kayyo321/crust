/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

struct fi {
    float f;
    int i;
};

struct dd {
    double a;
    double b;
};

struct fff {
    float x;
    float y;
    float z;
};

struct cd {
    char c;
    double d;
};

struct fi scale_fi(struct fi v, float k) {
    v.f *= k;
    v.i *= 3;
    return v;
}

struct dd swap_dd(struct dd v) {
    return (struct dd){v.b, v.a};
}

struct fff cross(struct fff a, struct fff b) {
    return (struct fff){
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    };
}

double use_cd(struct cd v, int extra, double more) {
    return v.c + v.d + extra + more;
}

int main(void) {
    struct fi a = scale_fi((struct fi){1.5f, 7}, 2.0f);
    printf("%.2f %d\n", a.f, a.i);
    struct dd b = swap_dd((struct dd){1.25, -2.5});
    printf("%.3f %.3f\n", b.a, b.b);
    struct fff c = cross((struct fff){1, 0, 0}, (struct fff){0, 1, 0});
    printf("%.1f %.1f %.1f\n", c.x, c.y, c.z);
    printf("%.2f\n", use_cd((struct cd){1, 0.5}, 2, 0.25));
    return 0;
}

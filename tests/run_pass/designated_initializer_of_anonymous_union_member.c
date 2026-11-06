/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <stdint.h>

union word {
    uint32_t value;
    uint8_t bytes[4];
};

enum ShapeKind {
    ShapeKindCircle,
    ShapeKindRect,
};

struct shape {
    enum ShapeKind kind;
    union {
        struct {
            double radius;
        } circle;
        struct {
            double w;
            double h;
        } rect;
    };
};

double area(struct shape s) {
    switch (s.kind) {
    case ShapeKindCircle:
        return 3.0 * s.circle.radius * s.circle.radius;
    case ShapeKindRect:
        return s.rect.w * s.rect.h;
    }
    return 0.0;
}

int main(void) {
    union word w;
    w.value = 0x01020304u;
    printf("%u %u\n", (unsigned)w.bytes[0], (unsigned)w.bytes[3]);
    printf("%zu\n", sizeof(union word));
    struct shape c = {.kind = ShapeKindCircle, .circle = {2.0}};
    struct shape r = {.kind = ShapeKindRect, .rect = {3.0, 4.5}};
    printf("%.1f %.1f\n", area(c), area(r));
    return 0;
}

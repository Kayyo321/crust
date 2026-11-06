/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

struct point {
    int x;
    int y;
};

static int sum(const int *values, int n) {
    int total = 0;
    for (int i = 0; i < n; i++) {
        total += values[i];
    }
    return total;
}

static int dot(struct point a, struct point b) {
    return a.x * b.x + a.y * b.y;
}

int main(void) {
    printf("%d\n", sum((int[]){1, 2, 3, 4}, 4));
    printf("%d\n", dot((struct point){2, 3}, (struct point){.y = 5, .x = 4}));
    struct point *p = &(struct point){7, 8};
    p->x += 1;
    printf("%d %d\n", p->x, p->y);
    const char **names = (const char *[]){"x", "y", "z"};
    printf("%s%s%s\n", names[2], names[1], names[0]);
    int *arr = (int[3]){0};
    arr[1] = 9;
    printf("%d %d %d\n", arr[0], arr[1], arr[2]);
    return 0;
}

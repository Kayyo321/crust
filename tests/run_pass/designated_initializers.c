/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

struct config {
    int width;
    int height;
    const char *title;
    double scale;
    int flags[4];
};

struct nested {
    struct config cfg;
    int id;
};

int main(void) {
    struct config c = {.title = "main", .height = 480, .flags = {[2] = 7}, .width = 640};
    printf("%d %d %s %.1f\n", c.width, c.height, c.title, c.scale);
    printf("%d %d %d %d\n", c.flags[0], c.flags[1], c.flags[2], c.flags[3]);
    int table[10] = {[1] = 1, [5] = 5, 6, [9] = 9};
    for (int i = 0; i < 10; i++) {
        printf("%d", table[i]);
    }
    printf("\n");
    struct nested n = {.cfg.scale = 2.5, .cfg.flags[3] = 3, .id = 11};
    printf("%.1f %d %d\n", n.cfg.scale, n.cfg.flags[3], n.id);
    struct config empty = {};
    printf("%d %d\n", empty.width, empty.title == nullptr);
    return 0;
}

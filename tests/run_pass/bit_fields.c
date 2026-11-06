/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

struct flags {
    unsigned int ready : 1;
    unsigned int mode : 3;
    signed int delta : 4;
    unsigned int count : 8;
};

int main(void) {
    struct flags f = {0};
    f.ready = 1;
    f.mode = 5;
    f.delta = -3;
    f.count = 255;
    printf("%u %u %d %u\n", f.ready, f.mode, f.delta, f.count);
    f.count += 1;
    f.mode = 9;
    printf("%u %u\n", f.count, f.mode);
    f.delta = 7;
    f.delta += 1;
    printf("%d\n", f.delta);
    printf("%d\n", (int)(sizeof(struct flags) <= sizeof(unsigned int)));
    return 0;
}

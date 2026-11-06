/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

struct big {
    long values[8];
    char tag[16];
};

struct big fill(long start, const char *tag) {
    struct big b;
    for (int i = 0; i < 8; i++) {
        b.values[i] = start + i * i;
    }
    snprintf(b.tag, sizeof b.tag, "%s", tag);
    return b;
}

long total(struct big b) {
    long sum = 0;
    for (int i = 0; i < 8; i++) {
        sum += b.values[i];
    }
    return sum;
}

struct big twice(struct big b) {
    for (int i = 0; i < 8; i++) {
        b.values[i] *= 2;
    }
    b.tag[0] = 'T';
    return b;
}

int main(void) {
    struct big a = fill(100, "first");
    struct big b = twice(a);
    printf("%ld %s\n", total(a), a.tag);
    printf("%ld %s\n", total(b), b.tag);
    printf("%ld %ld\n", b.values[0], b.values[7]);
    return 0;
}

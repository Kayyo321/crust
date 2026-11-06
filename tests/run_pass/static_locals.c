/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

static int next_id(void) {
    static int counter = 100;
    return counter++;
}

static const char *cached(int reset) {
    static char buf[8];
    static int calls;
    if (reset) {
        calls = 0;
    }
    calls++;
    snprintf(buf, sizeof buf, "c%d", calls);
    return buf;
}

int main(void) {
    printf("%d %d %d\n", next_id(), next_id(), next_id());
    printf("%s\n", cached(0));
    printf("%s\n", cached(0));
    printf("%s\n", cached(1));
    return 0;
}

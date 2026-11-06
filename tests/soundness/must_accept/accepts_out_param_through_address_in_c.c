/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UNINIT
#include <stdio.h>

static void produce(int *out) {
    *out = 21;
}

int main(void) {
    int parsed;
    if (sscanf("42", "%d", &parsed) != 1) {
        return 1;
    }
    int made;
    produce(&made);
    return parsed - 2 * made;
}

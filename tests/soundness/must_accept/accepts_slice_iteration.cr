/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-OOB
#include <stddef.h>

int count_positive(ref [int] values) {
    let mut n = 0;
    for (size_t i = 0; i < values.len; i++) {
        if (values[i] > 0) {
            n++;
        }
    }
    return n;
}

void negate_all(ref mut [int] values) {
    for (size_t i = 0; i < values.len; i++) {
        values[i] = -values[i];
    }
}

int main(void) {
    int data[6] = {3, -1, 4, -1, 5, -9};
    let before = count_positive(ref data);
    negate_all(ref mut data);
    let after = count_positive(ref data);
    return before * 10 + after - 33;
}

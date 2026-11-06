/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

int sum(ref [int] s) {
    let mut total = 0;
    for (size_t i = 0; i < s.len; i++) {
        total += s[i];
    }
    return total;
}

void fill_squares(ref mut [int] s) {
    for (size_t i = 0; i < s.len; i++) {
        s[i] = (int)(i * i);
    }
}

int max_of(ref [int] s) {
    let mut best = s[0];
    for (size_t i = 1; i < s.len; i++) {
        if (s[i] > best) {
            best = s[i];
        }
    }
    return best;
}

int main(void) {
    int values[5] = {4, -1, 9, 2, 7};
    int squares[6] = {0};
    fill_squares(ref mut squares);
    let view: ref [int] = ref values;
    unsafe {
        // SAFETY: the format specifiers match the argument types.
        printf("%zu %d %d\n", view.len, sum(view), max_of(ref values));
        printf("%d %d %d\n", sum(ref squares), squares[5], max_of(ref squares));
    }
    let mut idx = 0;
    for (size_t i = 0; i < 5; i++) {
        if (values[i] == 9) {
            idx = (int)i;
        }
    }
    values[idx] = 0;
    unsafe {
        // SAFETY: the format specifier matches the int argument.
        printf("%d\n", sum(ref values));
    }
    return 0;
}

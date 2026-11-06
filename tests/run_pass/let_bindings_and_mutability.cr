/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

[[crust::safe]] int puts(const char *s);

int main(void) {
    let count = 3;
    let mut total = 0;
    for (int i = 0; i < count; i++) {
        total += i * 10;
    }
    let ratio: double = 1.5;
    let label: const char * = "done";
    let later: int;
    if (total > 20) {
        later = 1;
    } else {
        later = 2;
    }
    let mut chars = 'a';
    chars += 2;
    int legacy = 7;
    legacy *= 2;
    unsafe {
        // SAFETY: each format specifier matches the type of its argument.
        printf("%d %d %.2f %d %c %d\n", count, total, ratio * 2, later, chars, legacy);
    }
    puts(label);
    return 0;
}

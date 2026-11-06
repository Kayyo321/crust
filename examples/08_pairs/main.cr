/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// A .cr file that uses the pair. Old-school sources can also import
// modules.
//
//   crust main.cr counter.cr -o counter && ./counter
#include <stdio.h>
#include "counter.hr"

[[crust::safe]] int printf(const char *format, ...);

int main(void) {
    let mut c = counter_new();
    let mut ticks = 0;
    while (counter_tick(ref mut c)) {
        ticks += 1;
    }
    int values[5] = { 0, 1, 2, 7, 1 };
    printf("ticks=%d limit=%d below=%zu\n", ticks, CounterLimit, counter_count_until_limit(ref values));
    return 0;
}

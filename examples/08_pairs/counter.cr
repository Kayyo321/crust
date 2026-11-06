/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// The source half of the pair. Changing a signature here without changing
// counter.hr is error CR1501.
#include "counter.hr"

struct counter counter_new(void) {
    return (struct counter){ .value = 0 };
}

bool counter_tick(ref mut struct counter c) {
    if (c->value >= CounterLimit) {
        return false;
    }
    c->value += 1;
    return true;
}

size_t counter_count_until_limit(ref [int] values) {
    size_t n = 0;
    while (n < values.len && values[n] < CounterLimit) {
        n += 1;
    }
    return n;
}

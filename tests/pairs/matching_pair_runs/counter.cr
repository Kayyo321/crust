/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include "counter.hr"

struct counter counter_new(int step) {
    return (struct counter){ .count = 0, .step = step };
}

void counter_tick(ref mut struct counter c) {
    c->count += c->step;
}

int counter_value(ref struct counter c) {
    return c->count;
}

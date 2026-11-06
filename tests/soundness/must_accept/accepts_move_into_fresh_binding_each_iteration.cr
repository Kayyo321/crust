/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
#include <stdio.h>

struct tracker;
void tracker_drop(struct tracker *t);

[[crust::drop(tracker_drop)]] struct tracker {
    int id;
};

void tracker_drop(struct tracker *t) {
    unsafe {
        // SAFETY: `t` points at the live tracker being dropped, and the
        // format specifier matches the int argument.
        printf("drop %d\n", t->id);
    }
}

struct tracker make(int id) {
    return (struct tracker){ .id = id };
}

void consume(struct tracker t) {
    (void)t;
}

int main(void) {
    let mut total = 0;
    for (int i = 0; i < 3; i++) {
        let fresh = make(i);
        let moved = fresh;
        total += moved.id;
    }
    return total - 3;
}

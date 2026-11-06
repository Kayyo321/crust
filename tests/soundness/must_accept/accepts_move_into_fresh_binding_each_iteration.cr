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

static int drop_count;
static int drop_id_sum;

void tracker_drop(struct tracker *t) {
    unsafe {
        // SAFETY: `t` points at the live tracker being dropped, and the
        // format specifier matches the int argument.
        printf("drop %d\n", t->id);
        drop_count += 1;
        drop_id_sum += t->id;
    }
}

struct tracker make(int id) {
    return (struct tracker){ .id = id };
}

void consume(struct tracker t) {
    (void)t;
}

int run(void) {
    let mut total = 0;
    for (int i = 0; i < 3; i++) {
        let fresh = make(i);
        let moved = fresh;
        total += moved.id;
    }
    return total - 3;
}

int main(void) {
    let status = run();
    let mut wrong = 0;
    unsafe {
        // SAFETY: the counters are only touched on this single thread.
        wrong = drop_count != 3 || drop_id_sum != 3;
    }
    return status != 0 || wrong;
}

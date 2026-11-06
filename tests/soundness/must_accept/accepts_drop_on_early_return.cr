/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-LEAK
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

int check(int v) {
    let guard = make(v);
    if (v < 0) {
        return -1;
    }
    let second = make(v + 100);
    if (v == 0) {
        return 0;
    }
    return 1;
}

int run(void) {
    check(-5);
    check(0);
    check(7);
    return 0;
}

int main(void) {
    let status = run();
    let mut wrong = 0;
    unsafe {
        // SAFETY: the counters are only touched on this single thread.
        wrong = drop_count != 5 || drop_id_sum != 209;
    }
    return status != 0 || wrong;
}

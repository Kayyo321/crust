/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-DF
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

int run(int argc, char **argv) {
    (void)argv;
    let mut t = make(1);
    if (argc > 0) {
        consume(t);
        t = make(2);
    }
    return t.id - 2;
}

int main(int argc, char **argv) {
    let status = run(argc, argv);
    let mut wrong = 0;
    unsafe {
        // SAFETY: the counters are only touched on this single thread.
        wrong = drop_count != 2 || drop_id_sum != 3;
    }
    return status != 0 || wrong;
}

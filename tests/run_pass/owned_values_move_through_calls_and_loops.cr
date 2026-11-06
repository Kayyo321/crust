/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
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

void say(const char *s) {
    unsafe {
        // SAFETY: `s` is a valid string, and printf is given a fixed format.
        printf("%s\n", s);
    }
}

struct tracker relabel(struct tracker t, int id) {
    say("relabel");
    let next = make(id);
    return next;
}

struct tracker pass_through(struct tracker t) {
    return t;
}

int main(void) {
    let a = make(1);
    let b = relabel(a, 2);
    let c = pass_through(b);
    say("loop");
    for (int i = 0; i < 3; i++) {
        let item = make(10 + i);
        let holder = item;
        if (i == 1) {
            continue;
        }
        say("iteration end");
    }
    say("main ends");
    return c.id - 2;
}

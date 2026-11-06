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

void consume(struct tracker t) {
    say("consume");
}

void maybe(bool give) {
    let t = make(10);
    if (give) {
        consume(t);
    }
    say("maybe ends");
}

void reinit(void) {
    let mut t = make(20);
    consume(t);
    t = make(21);
    t = make(22);
    say("reinit ends");
}

int main(void) {
    maybe(true);
    maybe(false);
    reinit();
    return 0;
}

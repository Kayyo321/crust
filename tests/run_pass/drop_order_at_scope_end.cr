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

int main(void) {
    let a = make(1);
    let b = make(2);
    {
        let inner = make(3);
        say("inner scope ends");
    }
    let c = make(4);
    consume(b);
    let d = a;
    say("main ends");
    return 0;
}

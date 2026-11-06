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

int find(int target) {
    let a = make(1);
    for (int i = 0; i < 5; i++) {
        let step = make(100 + i);
        if (i == target) {
            say("found");
            return i;
        }
    }
    let b = make(2);
    say("not found");
    return -1;
}

int main(void) {
    find(1);
    find(9);
    return 0;
}

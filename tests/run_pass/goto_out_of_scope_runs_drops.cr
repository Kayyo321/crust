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

int run(int mode) {
    let a = make(1);
    if (mode == 0) {
        goto done;
    }
    {
        let b = make(2);
        if (mode == 1) {
            goto done;
        }
        say("inner end");
    }
done:
    say("done");
    return mode;
}

int main(void) {
    run(0);
    run(1);
    run(2);
    return 0;
}

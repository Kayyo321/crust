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

struct pair;
void pair_drop(struct pair *p);

[[crust::drop(pair_drop)]] struct pair {
    struct tracker first;
    struct tracker second;
};

void pair_drop(struct pair *p) {
    unsafe {
        // SAFETY: `p` points at the live pair being dropped.
        printf("drop pair %d %d\n", p->first.id, p->second.id);
    }
}

struct bundle {
    int tag;
    struct tracker left;
    struct tracker right;
};

int main(void) {
    let p = (struct pair){ .first = make(1), .second = make(2) };
    let b = (struct bundle){ .tag = 7, .left = make(3), .right = make(4) };
    let tag = b.tag;
    let mut c = (struct bundle){ .tag = 8, .left = make(5), .right = make(6) };
    let taken = c.left;
    c.left = make(9);
    unsafe {
        // SAFETY: the format specifiers match the int arguments.
        printf("tag %d %d\n", tag, taken.id);
    }
    return 0;
}

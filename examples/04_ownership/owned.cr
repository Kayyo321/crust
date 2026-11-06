/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Owned values, moves, and drops (LANGUAGE.md, sections 4 and 8;
// MEMORY_SAFETY.md, section 4). Every owned value is released exactly once,
// on every path (G-LEAK, G-DF).
//
//   crust owned.cr -o owned && ./owned

#include <stdio.h>
#include <stdlib.h>

[[crust::safe]] int printf(const char *format, ...);

// `[[crust::drop(f)]]` makes a type owned. The drop function takes a
// pointer (or `ref mut`) to the value being released.
[[crust::drop(buffer_free)]]
struct buffer {
    char *data;
    size_t len;
    int id;
};

void buffer_free(ref mut struct buffer b) {
    printf("drop buffer %d\n", b->id);
    unsafe {
        // SAFETY: `data` came from calloc in buffer_new and is freed only
        // here, once, when the owning buffer is dropped.
        free(b->data);
    }
}

struct buffer buffer_new(int id, size_t len) {
    unsafe {
        // SAFETY: calloc returns zeroed memory or null; free accepts null.
        return (struct buffer){ .data = calloc(len, 1), .len = len, .id = id };
    }
}

// A struct with an owned member is owned too. It has no drop function of
// its own, so dropping it drops its members.
struct pair {
    struct buffer first;
    struct buffer second;
};

// Taking an owned value by value moves it in; it is dropped at the end.
void consume(struct buffer b) {
    printf("consume %d\n", b.id);
}

// Returning moves the value out to the caller.
struct buffer pass_through(struct buffer b) {
    return b;
}

int early(bool leave) {
    let b = buffer_new(5, 8);
    if (leave) {
        return 1;                   // b is dropped here
    }
    consume(b);                     // or moved here
    return 0;
}

int main(void) {
    let a = buffer_new(1, 16);
    let moved = a;                  // `a` is moved; using it now is CR0382
    consume(moved);

    let b = buffer_new(2, 16);
    let c = pass_through(b);

    let d = buffer_new(3, 16);
    if (c.len > 100) {
        consume(d);                 // moved on one path only: a drop flag
    }                               // decides at the end of main

    let mut e = buffer_new(4, 4);
    e = buffer_new(40, 4);          // assigning drops the old value first

    early(true);
    early(false);

    let p = (struct pair){ .first = buffer_new(6, 1), .second = buffer_new(7, 1) };
    consume(p.second);              // a field moves out; the rest is dropped later
    printf("end of main\n");
    return 0;                       // drops: p.first, e, d, c (reverse order)
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <stdlib.h>

struct buffer;
void buffer_drop(struct buffer *b);

[[crust::drop(buffer_drop)]] struct buffer {
    int *data;
    size_t len;
};

void buffer_drop(struct buffer *b) {
    unsafe {
        // SAFETY: `b->data` came from malloc in buffer_new and is freed once,
        // here, when the owning buffer is dropped.
        free(b->data);
    }
}

struct buffer buffer_new(size_t len) {
    int *data;
    unsafe {
        // SAFETY: the size cannot overflow for the small lengths used here.
        data = malloc(len * sizeof(int));
        if (data == nullptr) {
            exit(1);
        }
    }
    return (struct buffer){ .data = data, .len = len };
}

void buffer_set(ref mut struct buffer b, size_t i, int v) {
    if (i >= b->len) {
        return;
    }
    unsafe {
        // SAFETY: `i < b->len`, and `data` has `len` elements.
        b->data[i] = v;
    }
}

int buffer_sum(ref struct buffer b) {
    let mut total = 0;
    unsafe {
        // SAFETY: every index in [0, len) is inside the allocation.
        for (int *p = b->data; p < b->data + b->len; p++) {
            total += *p;
        }
    }
    return total;
}

int main(void) {
    let mut buf = buffer_new(4);
    for (size_t i = 0; i < 4; i++) {
        buffer_set(ref mut buf, i, (int)(i + 1) * 3);
    }
    buffer_set(ref mut buf, 99, 1000);
    let moved = buf;
    unsafe {
        // SAFETY: the format specifiers match the argument types.
        printf("%zu %d\n", moved.len, buffer_sum(ref moved));
    }
    return 0;
}

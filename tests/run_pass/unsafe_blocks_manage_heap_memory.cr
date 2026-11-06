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
        // SAFETY: `b->data` came from calloc in buffer_new and is freed once,
        // here, when the owning buffer is dropped.
        printf("free %zu\n", b->len);
        free(b->data);
    }
}

struct buffer buffer_new(size_t len) {
    int *data;
    unsafe {
        // SAFETY: calloc checks `len * sizeof(int)` for overflow.
        data = calloc(len, sizeof(int));
        if (data == nullptr) {
            exit(1);
        }
    }
    return (struct buffer){ .data = data, .len = len };
}

struct buffer buffer_counting(size_t len, int step) {
    let b = buffer_new(len);
    unsafe {
        // SAFETY: `data` has `len` elements, and `i < len`.
        for (size_t i = 0; i < b.len; i++) {
            b.data[i] = (int)(i + 1) * step;
        }
    }
    return b;
}

int buffer_sum(ref struct buffer b) {
    let mut total = 0;
    unsafe {
        // SAFETY: `p` walks from the first element to one past the last.
        for (int *p = b->data; p < b->data + b->len; p++) {
            total += *p;
        }
    }
    return total;
}

int main(void) {
    let buf = buffer_counting(4, 3);
    let first_total = buffer_sum(ref buf);
    let moved = buf;
    let small = buffer_new(2);
    unsafe {
        // SAFETY: the format specifiers match the argument types.
        printf("%zu %d %d %d\n", moved.len, first_total, buffer_sum(ref moved), buffer_sum(ref small));
    }
    return 0;
}

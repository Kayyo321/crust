/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-DF
struct box;
void box_drop(struct box *b);

[[crust::drop(box_drop)]] struct box {
    int value;
};

void box_drop(struct box *b) {
    (void)b;
}

void consume_two(struct box a, struct box b) {
    (void)a;
    (void)b;
}

int main(void) {
    let a = (struct box){ .value = 1 };
    consume_two(a, a);   //~ ERROR CR0382
    return 0;
}

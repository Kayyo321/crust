/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
struct box;
void box_drop(struct box *b);

[[crust::drop(box_drop)]] struct box {
    int value;
};

void box_drop(struct box *b) {
    (void)b;
}

void consume(struct box b) {
    (void)b;
}

int main(void) {
    let b = (struct box){ .value = 1 };
    consume(b);
    return b.value;   //~ ERROR CR0382
}

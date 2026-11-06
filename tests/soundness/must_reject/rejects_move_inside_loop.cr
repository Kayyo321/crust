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

void consume(struct box b) {
    (void)b;
}

int main(void) {
    let b = (struct box){ .value = 1 };
    for (int i = 0; i < 2; i++) {
        consume(b);   //~ ERROR CR0382
    }
    return 0;
}

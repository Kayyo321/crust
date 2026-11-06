/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-LEAK
struct box;
void box_drop(struct box *b);

[[crust::drop(box_drop)]] struct box {
    int value;
};

void box_drop(struct box *b) {
    (void)b;
}

union slot {
    struct box owned;   //~ ERROR CR0702
    long raw;
};

int main(void) {
    return 0;
}

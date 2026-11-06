/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
struct box;
void box_drop(struct box *b);

[[crust::drop(box_drop)]] struct box {
    int value;
};

void box_drop(struct box *b) {
    (void)b;
}

union holder {
    struct box b;   //~ ERROR CR0702
    int raw;
};

int main(void) {
    return 0;
}

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

struct pair;
void pair_drop(struct pair *p);

[[crust::drop(pair_drop)]] struct pair {
    struct box first;
    struct box second;
};

void pair_drop(struct pair *p) {
    (void)p;
}

int main(void) {
    let p = (struct pair){ .first = { .value = 1 }, .second = { .value = 2 } };
    let first = p.first;   //~ ERROR CR0509
    return first.value;
}

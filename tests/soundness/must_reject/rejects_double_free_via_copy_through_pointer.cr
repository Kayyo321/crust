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

int main(void) {
    let mut a = (struct box){ .value = 1 };
    struct box *p = &a;
    struct box copy = *p;   //~ ERROR CR0507
    return copy.value;
}

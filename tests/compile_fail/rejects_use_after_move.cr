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

int take(struct box b) {
    return b.value;
}

int main(void) {
    let b = (struct box){ .value = 1 };
    let moved = b;
    return take(b) + moved.value;   //~ ERROR CR0382
}

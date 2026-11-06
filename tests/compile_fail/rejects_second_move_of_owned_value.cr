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

int main(void) {
    let b = (struct box){ .value = 1 };
    struct box copy = b;
    let again = b;   //~ ERROR CR0382
    return copy.value + again.value;
}

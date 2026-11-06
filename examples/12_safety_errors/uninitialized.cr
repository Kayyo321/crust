/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// G-UNINIT and immutability: definite initialization on every path.

int pick(bool flag) {
    int value;
    if (flag) {
        value = 1;
    }
    return value;                   //~ ERROR CR0381
}

int sign(int v) {
    if (v > 0) {
        return 1;
    }
}                                   //~ ERROR CR0381

int main(void) {
    let limit = 10;
    limit = 20;                     //~ ERROR CR0384
    return pick(true) + sign(limit);
}

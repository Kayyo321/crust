/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-OOB
int pick(int i) {
    int table[4] = {10, 20, 30, 40};
    return table[i];
}

int main(int argc, char **argv) {
    (void)argv;
    let i = argc + 2;
    return pick(i) - 40;
}

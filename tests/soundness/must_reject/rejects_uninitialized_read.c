/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UNINIT
int main(int argc, char **argv) {
    (void)argv;
    int total;
    for (int i = 0; i < argc; i++) {
        total = i;
    }
    return total;   //~ ERROR CR0381
}

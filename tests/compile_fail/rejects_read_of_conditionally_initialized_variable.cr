/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int main(int argc, char **argv) {
    (void)argv;
    int x;
    if (argc > 1) {
        x = 1;
    }
    return x;   //~ ERROR CR0381
}

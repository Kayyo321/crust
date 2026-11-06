/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UNINIT
int main(int argc, char **argv) {
    (void)argv;
    int value;
    switch (argc) {
    case 0:
        value = 10;
        break;
    case 1:
        value = 0;
        break;
    default:
        value = 20;
        break;
    }
    let mut other: int;
    if (value > 5) {
        other = 1;
    } else {
        other = 0;
    }
    return value + other;
}

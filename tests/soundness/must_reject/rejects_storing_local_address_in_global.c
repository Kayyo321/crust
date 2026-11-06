/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
static int *last_seen;

static void remember(void) {
    int local = 5;
    last_seen = &local;   //~ ERROR CR0521
}

int main(void) {
    remember();
    return 0;
}

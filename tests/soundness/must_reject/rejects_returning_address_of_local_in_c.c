/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
int *make_counter(void) {
    int counter = 0;
    return &counter;   //~ ERROR CR0515
}

int main(void) {
    return *make_counter();
}

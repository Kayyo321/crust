/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
// A read whose value is discarded is still a use of the borrow.
int main(void) {
    ref int view;
    {
        let short_lived = 5;
        view = ref short_lived;
    }   //~ ERROR CR0597
    (void)*view;
    return 0;
}

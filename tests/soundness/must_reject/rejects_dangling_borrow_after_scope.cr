/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
int main(void) {
    let mut view: ref int;
    {
        let inner = 1;
        view = ref inner;
    }   //~ ERROR CR0597
    return *view;
}

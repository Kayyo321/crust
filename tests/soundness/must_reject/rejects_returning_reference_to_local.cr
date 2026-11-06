/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
ref int first_or_local(ref int p) {
    let local = *p + 1;
    return ref local;   //~ ERROR CR0515
}

int main(void) {
    let x = 1;
    return *first_or_local(ref x);
}

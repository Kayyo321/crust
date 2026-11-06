/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
ref int dangle(ref int unused) {
    let x = *unused;
    return ref x;   //~ ERROR CR0515
}

int main(void) {
    let y = 1;
    return *dangle(ref y);
}

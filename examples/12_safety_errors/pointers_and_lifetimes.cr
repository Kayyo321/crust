/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// Raw pointers whose origin is unknown, and borrows at function
// boundaries. A parameter's pointee is not tracked, so dereferencing a raw
// pointer parameter needs `unsafe`; a borrow parameter needs nothing.

int first(int *p) {
    return *p;                      //~ ERROR CR0133
}

int first_borrowed(ref int p) {
    return *p;
}

ref int pick(ref int a, ref int b) {        //~ ERROR CR0106
    return a;
}

ref 'a int keep(ref 'a int a, ref 'b int b) {
    (void)a;
    return b;                       //~ ERROR CR0623
}

int main(void) {
    int x = 1;
    return first_borrowed(ref x);
}

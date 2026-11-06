/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
ref [int] window(ref [int] input) {
    int copy[3] = {0, 0, 0};
    if (input.len > 0) {
        copy[0] = input[0];
    }
    return ref copy;   //~ ERROR CR0515
}

int main(void) {
    int data[2] = {1, 2};
    let w = window(ref data);
    return w[0];
}

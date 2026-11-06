/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// guarantee: G-UAF
static void bump(int *p) {
    *p += 1;
}

int main(void) {
    int x = 1;
    int *p = &x;
    *p = 5;
    bump(&x);
    int arr[3] = {0, 0, 0};
    int *q = &arr[1];
    *q = x;
    return arr[1] - 6;
}

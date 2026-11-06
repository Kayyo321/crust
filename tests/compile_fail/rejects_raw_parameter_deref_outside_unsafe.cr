/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int read(int *p) {
    return *p;   //~ ERROR CR0133
}

int main(void) {
    int x = 0;
    return read(&x);
}

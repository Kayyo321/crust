/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
int *saved;

void keep(void) {
    int local = 1;
    saved = &local;   //~ ERROR CR0521
}

int main(void) {
    keep();
    return 0;
}

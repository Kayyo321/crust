/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

int crust_add(int a, int b);
long crust_sum(const int *values, unsigned long n);

int main(void) {
    int values[4] = {1, 2, 3, 4};
    printf("%d %ld\n", crust_add(40, 2), crust_sum(values, 4));
    return 0;
}

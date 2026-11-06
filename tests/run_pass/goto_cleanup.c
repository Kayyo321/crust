/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <stdlib.h>

static int process(int fail_at) {
    int result = -1;
    int *a = malloc(sizeof(int) * 4);
    if (!a) {
        goto out;
    }
    if (fail_at == 1) {
        goto free_a;
    }
    int *b = malloc(sizeof(int) * 4);
    if (!b) {
        goto free_a;
    }
    if (fail_at == 2) {
        goto free_b;
    }
    a[0] = 20;
    b[0] = 22;
    result = a[0] + b[0];
free_b:
    free(b);
free_a:
    free(a);
out:
    return result;
}

int main(void) {
    printf("%d %d %d\n", process(0), process(1), process(2));
    int i = 0;
loop:
    if (i < 5) {
        printf("%d", i);
        i++;
        goto loop;
    }
    printf("\n");
    return 0;
}

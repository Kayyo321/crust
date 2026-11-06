/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <stdlib.h>

typedef int (*binary_op)(int, int);

static int add(int a, int b) {
    return a + b;
}

static int sub(int a, int b) {
    return a - b;
}

static int mul(int a, int b) {
    return a * b;
}

static int compare_ints(const void *a, const void *b) {
    int x = *(const int *)a;
    int y = *(const int *)b;
    return (x > y) - (x < y);
}

struct op_entry {
    char symbol;
    binary_op fn;
};

static binary_op pick(char c) {
    return c == '+' ? add : c == '-' ? sub : mul;
}

static int apply_twice(int (*f)(int, int), int a, int b) {
    return f(f(a, b), b);
}

int main(void) {
    struct op_entry table[] = {{'+', add}, {'-', sub}, {'*', mul}};
    for (size_t i = 0; i < sizeof table / sizeof table[0]; i++) {
        printf("7 %c 3 = %d\n", table[i].symbol, table[i].fn(7, 3));
    }
    printf("%d\n", pick('-')(10, 4));
    printf("%d\n", apply_twice(mul, 2, 5));
    int values[] = {5, -2, 9, 0, 3};
    qsort(values, 5, sizeof values[0], compare_ints);
    for (int i = 0; i < 5; i++) {
        printf("%d%c", values[i], i == 4 ? '\n' : ' ');
    }
    return 0;
}

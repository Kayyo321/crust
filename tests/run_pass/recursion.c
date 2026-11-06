/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

static long fib(int n) {
    return n < 2 ? n : fib(n - 1) + fib(n - 2);
}

static int ackermann(int m, int n) {
    if (m == 0) {
        return n + 1;
    }
    if (n == 0) {
        return ackermann(m - 1, 1);
    }
    return ackermann(m - 1, ackermann(m, n - 1));
}

static bool is_odd(unsigned n);

static bool is_even(unsigned n) {
    return n == 0 ? true : is_odd(n - 1);
}

static bool is_odd(unsigned n) {
    return n == 0 ? false : is_even(n - 1);
}

static void hanoi(int n, char from, char to, char via, int *moves) {
    if (n == 0) {
        return;
    }
    hanoi(n - 1, from, via, to, moves);
    *moves += 1;
    hanoi(n - 1, via, to, from, moves);
}

int main(void) {
    printf("%ld\n", fib(25));
    printf("%d\n", ackermann(2, 3));
    printf("%d %d\n", is_even(10), is_odd(7));
    int moves = 0;
    hanoi(10, 'a', 'c', 'b', &moves);
    printf("%d\n", moves);
    return 0;
}

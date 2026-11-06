/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdarg.h>
#include <stdio.h>

int sum_ints(int count, ...) {
    va_list ap;
    va_start(ap, count);
    int total = 0;
    for (int i = 0; i < count; i++) {
        total += va_arg(ap, int);
    }
    va_end(ap);
    return total;
}

double mean(int count, ...) {
    va_list ap;
    va_start(ap, count);
    double total = 0;
    for (int i = 0; i < count; i++) {
        total += va_arg(ap, double);
    }
    va_end(ap);
    return total / count;
}

void log_line(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    printf("[log] ");
    vprintf(fmt, ap);
    va_end(ap);
}

long mixed(int n, ...) {
    va_list ap;
    va_start(ap, n);
    long acc = 0;
    for (int i = 0; i < n; i++) {
        acc += va_arg(ap, long);
        acc += (long)va_arg(ap, double);
        acc += *va_arg(ap, const char *) - '0';
    }
    va_end(ap);
    return acc;
}

int main(void) {
    printf("%d\n", sum_ints(5, 1, 2, 3, 4, 5));
    printf("%.2f\n", mean(3, 1.0, 2.5, 4.0));
    log_line("%s=%d, %s=%.1f\n", "a", 1, "b", 2.5);
    printf("%ld\n", mixed(2, 10L, 2.9, "3", 100L, 4.1, "7"));
    return 0;
}

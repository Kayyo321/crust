/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <float.h>

// Values here are exact in double precision too, so the output is the same
// under valgrind, which computes long double arithmetic at double precision.

static long double half(long double v) {
    return v / 2;
}

struct measured {
    char unit;
    long double amount;
};

static long double total(struct measured a, struct measured b) {
    return a.amount + b.amount;
}

int main(void) {
    printf("%.5Lf\n", half(10.5L));
    long double sum = 0;
    for (int i = 1; i <= 8; i++) {
        sum += 1.0L / (1 << i);
    }
    printf("%.10Lf\n", sum);
    printf("%Lg %Le\n", 1024.0L * 1024.0L, 0.125L);
    printf("%.1Lf\n", total((struct measured){'m', 1.25L}, (struct measured){'m', 2.5L}));
    long double big = 1.0L;
    for (int i = 0; i < 70; i++) {
        big *= 2;
    }
    printf("%.0Lf\n", big);
    printf("%d %zu\n", LDBL_MANT_DIG, sizeof(long double));
    printf("%.3f %lld\n", (double)(3.5L * 2), (long long)-7.75L);
    printf("%d %d\n", 1.5L > 1.25L, (long double)1 == 1.0);
    return 0;
}

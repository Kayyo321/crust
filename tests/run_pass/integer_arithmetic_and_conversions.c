/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <stdint.h>
#include <inttypes.h>
#include <limits.h>

int main(void) {
    uint64_t big = UINT64_MAX;
    printf("%" PRIu64 "\n", big);
    big += 2;
    printf("%" PRIu64 "\n", big);
    int64_t neg = INT64_MIN + 1;
    printf("%" PRId64 "\n", neg);
    unsigned u = 0;
    u -= 1;
    printf("%u\n", u);
    printf("%d %d\n", -7 / 2, -7 % 2);
    printf("%lld\n", (long long)INT_MAX * 4);
    unsigned char c = (unsigned char)300;
    signed char s = (signed char)200;
    printf("%u %d\n", c, s);
    printf("%d\n", -1 < 1u);
    printf("%d\n", (int)(-1L < 1u));
    uint32_t x = 0x80000000u;
    printf("%u %u\n", x >> 31, x << 1);
    int64_t shifted = (int64_t)1 << 40;
    printf("%" PRId64 "\n", shifted);
    double d = 3.99;
    printf("%d %d\n", (int)d, (int)-d);
    printf("%" PRIu64 "\n", (uint64_t)1e19);
    uint16_t h = 65535;
    h++;
    printf("%u\n", h);
    printf("%" PRIx64 "\n", (uint64_t)0xDEADBEEFCAFEBABEull);
    printf("%zu %zu\n", sizeof(long), sizeof(long long));
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* Layout test: prints sizeof, alignof, and offsetof for each shared type.
   Compiled by crust and by clang; the outputs must be identical. */
#include <stdio.h>
#include <stddef.h>
#include "shapes.h"

#define SHOW(T) printf("%-18s size %2zu align %2zu\n", #T, sizeof(T), alignof(T))
#define OFF(T, m) printf("  %-16s offset %2zu\n", #m, offsetof(T, m))

int main(void) {
    SHOW(struct s_char3);
    SHOW(struct s_int2);
    SHOW(struct s_float3);
    SHOW(struct s_mixed);
    OFF(struct s_mixed, d);
    SHOW(struct s_mixed2);
    OFF(struct s_mixed2, i);
    SHOW(struct s_ptr);
    SHOW(struct s_big);
    SHOW(struct s_ld);
    SHOW(struct s_bits);
    OFF(struct s_bits, d);
    SHOW(struct s_nested);
    OFF(struct s_nested, s);
    OFF(struct s_nested, i);
    SHOW(union u_any);
    SHOW(struct s_arr);
    OFF(struct s_arr, tag);
    SHOW(enum e_small);
    SHOW(long double);
    SHOW(_Bool);
    SHOW(max_align_t);
    struct s_bits b = { 5, -9, 1000000, 'x' };
    printf("bits %u %d %u %c\n", b.a, b.b, b.c, b.d);
    return 0;
}

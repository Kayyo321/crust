/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* The C half of the call tests. Built by clang, linked with the Crust half. */
#include <stdarg.h>
#include <stdio.h>
#include "shapes.h"

int c_sum_char3(struct s_char3 s) { return s.a + s.b + s.c; }
struct s_int2 c_swap_int2(struct s_int2 s) { return (struct s_int2){ s.b, s.a }; }
float c_sum_float3(struct s_float3 s) { return s.a + s.b + s.c; }
struct s_double2 c_scale(struct s_double2 s, double k) { return (struct s_double2){ s.a * k, s.b * k }; }
double c_mixed(struct s_mixed m, struct s_mixed2 n) { return m.i + m.d + n.d + n.i; }
long c_big(struct s_big b) { return b.a + b.b + b.c + b.d + b.e; }
struct s_big c_make_big(long x) { return (struct s_big){ x, x + 1, x + 2, x + 3, x + 4 }; }
long double c_ld(struct s_ld s, long double y) { return s.x + y; }
int c_varargs(int n, ...) {
    va_list ap;
    va_start(ap, n);
    int total = 0;
    for (int i = 0; i < n; i++) {
        total += va_arg(ap, int);
        total += (int)va_arg(ap, double);
    }
    va_end(ap);
    return total;
}
int c_apply(binop f, int a, int b) { return f(a, b); }
unsigned char c_uchar(unsigned char a, signed char b, short c, _Bool d) { return (unsigned char)(a + b + c + d); }
/* Many arguments: some structs spill to the stack. */
double c_many(struct s_double2 a, struct s_double2 b, struct s_double2 c, struct s_double2 d, struct s_double2 e) {
    return a.a + b.a + c.a + d.a + e.a + a.b + b.b + c.b + d.b + e.b;
}

/* Calls into the Crust half. */
int crust_sum_char3(struct s_char3 s);
struct s_int2 crust_swap_int2(struct s_int2 s);
struct s_big crust_make_big(long x);
double crust_mixed(struct s_mixed m, struct s_mixed2 n);
float crust_sum_float3(struct s_float3 s);
long double crust_ld(struct s_ld s, long double y);
int crust_add(int a, int b);

void c_calls_crust(void) {
    printf("c->crust char3 %d\n", crust_sum_char3((struct s_char3){ 1, 2, 3 }));
    struct s_int2 s = crust_swap_int2((struct s_int2){ 7, 9 });
    printf("c->crust swap %d %d\n", s.a, s.b);
    struct s_big b = crust_make_big(10);
    printf("c->crust big %ld %ld\n", b.a, b.e);
    printf("c->crust mixed %.1f\n", crust_mixed((struct s_mixed){ 1, 2.5 }, (struct s_mixed2){ 3.5, 4 }));
    printf("c->crust float3 %.1f\n", crust_sum_float3((struct s_float3){ 1.5f, 2.5f, 3.0f }));
    printf("c->crust ld %.1Lf\n", crust_ld((struct s_ld){ 1.25L }, 2.0L));
    printf("c->crust fnptr %d\n", c_apply(crust_add, 20, 22));
}

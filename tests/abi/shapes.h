/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* Types shared by the ABI tests. Both compilers see the same definitions. */
#ifndef ABI_SHAPES_H
#define ABI_SHAPES_H
#include <stddef.h>
#include <stdint.h>

struct s_char3 { char a, b, c; };
struct s_int2 { int a, b; };
struct s_float2 { float a, b; };
struct s_float3 { float a, b, c; };
struct s_double2 { double a, b; };
struct s_mixed { int i; double d; };
struct s_mixed2 { double d; int i; };
struct s_ptr { const char *p; long n; };
struct s_big { long a, b, c, d, e; };
struct s_ld { long double x; };
struct s_bits { unsigned a : 3; int b : 7; unsigned c : 20; char d; };
struct s_nested { struct s_char3 c; short s; struct s_int2 i; };
union u_any { int i; double d; char c[12]; };
struct s_arr { int v[3]; char tag; };
enum e_small { ESmallA, ESmallB = 200 };

typedef int (*binop)(int, int);
#endif

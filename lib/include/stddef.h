/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* Freestanding <stddef.h> for Crust. System headers may ask for single
   types with __need_size_t and friends; this header defines all of them. */
#ifndef CRUST_STDDEF_TYPES
#define CRUST_STDDEF_TYPES
typedef __SIZE_TYPE__ size_t;
typedef __PTRDIFF_TYPE__ ptrdiff_t;
typedef __WCHAR_TYPE__ wchar_t;
typedef __WINT_TYPE__ wint_t;
typedef struct {
    long long max_align_ll;
    long double max_align_ld;
} max_align_t;
typedef typeof(nullptr) nullptr_t;
#endif
#ifndef NULL
#define NULL ((void *)0)
#endif
#ifndef offsetof
#define offsetof(type, member) __builtin_offsetof(type, member)
#endif
#ifndef unreachable
#define unreachable() __builtin_unreachable()
#endif
#define __STDC_VERSION_STDDEF_H__ 202311L
#undef __need_size_t
#undef __need_ptrdiff_t
#undef __need_wchar_t
#undef __need_wint_t
#undef __need_NULL
#undef __need_max_align_t
#undef __need_offsetof

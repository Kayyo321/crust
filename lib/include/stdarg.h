/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* Freestanding <stdarg.h> for Crust. */
#ifndef __GNUC_VA_LIST
#define __GNUC_VA_LIST
typedef __builtin_va_list __gnuc_va_list;
#endif
#ifdef __need___va_list
#undef __need___va_list
#else
#ifndef CRUST_STDARG_H
#define CRUST_STDARG_H
typedef __builtin_va_list va_list;
#define va_start(ap, ...) __builtin_va_start(ap, 0)
#define va_end(ap) __builtin_va_end(ap)
#define va_arg(ap, type) __builtin_va_arg(ap, type)
#define va_copy(dest, src) __builtin_va_copy(dest, src)
#define __va_copy(dest, src) __builtin_va_copy(dest, src)
#define __STDC_VERSION_STDARG_H__ 202311L
#endif
#endif

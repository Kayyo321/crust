/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* A C header for C code. Included from Crust, it is parsed as C, so Crust
   keywords such as `ref` and `import` are ordinary names here (D-032). */
#ifndef STATS_H
#define STATS_H
#include <stddef.h>

struct stats {
    double mean;
    double max;
};

/* `import` and `ref` are fine names in C. */
struct stats stats_of(const double *values, size_t n, int import, int ref);

#endif

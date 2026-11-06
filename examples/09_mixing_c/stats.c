/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* Plain C23, compiled by crust in C mode: checks stay on, but C calls,
   pointer arithmetic, and raw dereferences need no `unsafe` (D-037). */
#include "stats.h"

struct stats stats_of(const double *values, size_t n, int import, int ref) {
    struct stats s = { 0.0, values[0] };
    for (const double *p = values; p < values + n; p++) {
        s.mean += *p;
        if (*p > s.max) {
            s.max = *p;
        }
    }
    s.mean /= (double)n;
    (void)import;
    (void)ref;
    return s;
}

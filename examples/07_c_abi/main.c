/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* A C program using the Crust library through its generated header.
   See build.sh. */
#include <stdio.h>
#include "vecmath.h"

int main(void) {
    struct vec3 a = { 1, 2, 3 };
    struct vec3 b = vec3_scale(a, 2.0);
    printf("dot=%.1f y=%.1f\n", vec3_dot(&a, &b), vec3_get(b, AxisY));
    return 0;
}

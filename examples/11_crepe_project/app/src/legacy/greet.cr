/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include "greet.hr"

[[crust::safe]] int printf(const char *format, ...);

void greet(const char *who) {
    printf("hello, %s\n", who);
}

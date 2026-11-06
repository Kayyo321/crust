/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include "counter.hr"

int main(void) {
    let mut c = counter_new(5);
    counter_tick(ref mut c);
    counter_tick(ref mut c);
    unsafe {
        // SAFETY: the format specifier matches the int argument.
        printf("%d\n", counter_value(ref c));
    }
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include "limits.hr"

int main(void) {
    int values[MaxItems] = {1, 2, 5, 9};
    for (int i = 0; i < MaxItems; i++) {
        unsafe {
            // SAFETY: the format specifiers match the int arguments.
            printf("%d %d\n", scaled(values[i]), Clamp(values[i], Scale));
        }
    }
    return 0;
}

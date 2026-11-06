/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* In C mode, Crust's reserved words are ordinary names. With
   `--std=crust` the same file would be Crust, and this would not parse. */
#include <stdio.h>

int main(void) {
    int let = 1;
    int ref = 2;
    int import = 3;
    printf("c names=%d\n", let + ref + import);
    return 0;
}

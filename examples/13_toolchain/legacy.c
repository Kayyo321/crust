/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
/* Ordinary C that the checker rejects: it reads `n` before setting it.
   `--unchecked` compiles it anyway, with a warning, for migration only. */
#include <stdio.h>

int main(void) {
    int n;
    int unused = n;
    (void)unused;
    printf("unchecked build ran\n");
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

constexpr int Count = 4;

int main(void) {
    constexpr int Local = 2;
    int a[Count] = {1, 2, 3, 4};
    int b[Local] = {5, 6};
    printf("%d %d %zu\n", a[3], b[1], sizeof a / sizeof a[0]);
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include "limits.hr"

int scaled(int v) {
    return Clamp(v * Scale, MaxItems * Scale);
}

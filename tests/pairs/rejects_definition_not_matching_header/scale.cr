/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include "scale.hr"

long scale(int value, int factor) {   //~ ERROR CR1501
    return (long)value * factor;
}

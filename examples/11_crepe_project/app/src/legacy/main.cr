/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// An old-school source can import modules of its package too.
#include "greet.hr"

import { gcd } from "mathx";

int main(void) {
    greet("legacy tool");
    return gcd(10, 4) == 2 ? 0 : 1;
}

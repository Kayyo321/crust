/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
import { helper } from "util";

export int wrapped(void) {   //~ ERROR CR1409
    return helper();
}

int main(void) {
    return wrapped() - 1;
}

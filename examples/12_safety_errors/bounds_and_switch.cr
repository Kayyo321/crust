/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
// G-OOB and exhaustive switch.

enum Light {
    LightRed,
    LightAmber,
    LightGreen,
};

int wait_time(enum Light l) {
    switch (l) {                    //~ ERROR CR0004
    case LightRed:
        return 30;
    case LightGreen:
        return 0;
    }
    return 5;
}

int main(void) {
    int table[3] = { 1, 2, 3 };
    table[3] = 4;                   //~ ERROR CR0701
    return wait_time(LightAmber) + table[0];
}

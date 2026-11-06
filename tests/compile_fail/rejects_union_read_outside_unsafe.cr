/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
union bits {
    int i;
    float f;
};

int main(void) {
    union bits b;
    b.f = 1.0f;
    return b.i;   //~ ERROR CR0133
}

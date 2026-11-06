/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
struct opaque;
int main(void) {
    struct opaque o;   //~ ERROR CR1310
    return 0;
}

/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
struct pair {
    int a;
    int b;
};
int main(void) {
    struct pair p = {1, 2};
    return (int)p;   //~ ERROR CR1318
}

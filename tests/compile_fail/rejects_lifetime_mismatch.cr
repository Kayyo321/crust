/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
ref 'a int pick(ref 'a int a, ref 'b int b) {
    return b;   //~ ERROR CR0623
}

int main(void) {
    let x = 1;
    let y = 2;
    return *pick(ref x, ref y) - 1;
}

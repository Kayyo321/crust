/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
ref int pick(ref int a, ref int b) {   //~ ERROR CR0106
    return a;
}

int main(void) {
    let x = 1;
    let y = 2;
    return *pick(ref x, ref y) - 1;
}

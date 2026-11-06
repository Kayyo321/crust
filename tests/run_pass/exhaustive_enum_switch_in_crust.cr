/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

enum Token {
    TokenNumber,
    TokenPlus,
    TokenMinus,
};

struct item {
    enum Token kind;
    int value;
};

int eval(ref [struct item] items) {
    let mut acc = 0;
    let mut sign = 1;
    for (size_t i = 0; i < items.len; i++) {
        let it = items[i];
        switch (it.kind) {
        case TokenNumber:
            acc += sign * it.value;
            break;
        case TokenPlus:
            sign = 1;
            break;
        case TokenMinus:
            sign = -1;
            break;
        }
    }
    return acc;
}

int main(void) {
    struct item expr[5] = {
        { TokenNumber, 10 },
        { TokenMinus, 0 },
        { TokenNumber, 4 },
        { TokenPlus, 0 },
        { TokenNumber, 7 },
    };
    unsafe {
        // SAFETY: the format specifier matches the int argument.
        printf("%d\n", eval(ref expr));
    }
    return 0;
}

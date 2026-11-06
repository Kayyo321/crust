/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
enum Color {
    ColorRed,
    ColorGreen,
    ColorBlue,
};

int code(enum Color c) {
    switch (c) {   //~ ERROR CR0004
    case ColorRed:
        return 1;
    case ColorGreen:
        return 2;
    }
    return 0;
}

int main(void) {
    return code(ColorRed) - 1;
}

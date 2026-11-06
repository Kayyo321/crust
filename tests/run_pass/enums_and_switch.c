/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>

enum Weekday {
    WeekdayMon = 1,
    WeekdayTue,
    WeekdayWed,
    WeekdaySat = 6,
    WeekdaySun,
};

enum Small : unsigned char {
    SmallA = 200,
    SmallB,
};

const char *kind(enum Weekday d) {
    switch (d) {
    case WeekdayMon:
    case WeekdayTue:
    case WeekdayWed:
        return "work";
    case WeekdaySat:
    case WeekdaySun:
        return "rest";
    default:
        return "?";
    }
}

int classify(int n) {
    int r = 0;
    switch (n % 4) {
    case 0:
        r += 1;
        [[fallthrough]];
    case 1:
        r += 10;
        break;
    case 2:
        r = 100;
        break;
    default:
        r = -1;
    }
    return r;
}

int main(void) {
    printf("%d %d %d\n", WeekdayTue, WeekdayWed, WeekdaySun);
    printf("%s %s\n", kind(WeekdayTue), kind(WeekdaySun));
    printf("%d %d %d %d\n", classify(4), classify(5), classify(6), classify(7));
    printf("%d %zu\n", (int)SmallB, sizeof(enum Small));
    return 0;
}

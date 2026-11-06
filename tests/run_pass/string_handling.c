/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include <stddef.h>

static void reverse(char *s) {
    size_t n = strlen(s);
    for (size_t i = 0; i < n / 2; i++) {
        char t = s[i];
        s[i] = s[n - 1 - i];
        s[n - 1 - i] = t;
    }
}

static int count_words(const char *s) {
    int words = 0;
    bool in_word = false;
    for (; *s; s++) {
        if (isspace((unsigned char)*s)) {
            in_word = false;
        } else if (!in_word) {
            in_word = true;
            words++;
        }
    }
    return words;
}

int main(void) {
    char buf[64];
    strcpy(buf, "hello");
    strcat(buf, ", world");
    printf("%s %zu\n", buf, strlen(buf));
    reverse(buf);
    printf("%s\n", buf);
    printf("%d %d\n", strcmp("abc", "abd") < 0, strncmp("abcx", "abcy", 3));
    printf("%d\n", count_words("  the quick  brown fox "));
    char line[32];
    int n = snprintf(line, sizeof line, "%05d|%-4s|%x", 42, "ab", 255);
    printf("%s %d\n", line, n);
    const char *word = "crust";
    const char *found = strchr(word, 'u');
    printf("%s %td\n", found, found - word);
    printf("%s\n", strstr("needle in haystack", "hay"));
    char upper[16] = "mixed Case";
    for (char *p = upper; *p; p++) {
        *p = (char)toupper((unsigned char)*p);
    }
    printf("%s\n", upper);
    return 0;
}

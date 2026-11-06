/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/
#include <stdio.h>
#include <stdlib.h>

struct node {
    int value;
    struct node *next;
};

static struct node *push(struct node *head, int value) {
    struct node *n = malloc(sizeof *n);
    if (!n) {
        exit(1);
    }
    n->value = value;
    n->next = head;
    return n;
}

static struct node *reverse(struct node *head) {
    struct node *prev = nullptr;
    while (head) {
        struct node *next = head->next;
        head->next = prev;
        prev = head;
        head = next;
    }
    return prev;
}

static void free_list(struct node *head) {
    while (head) {
        struct node *next = head->next;
        free(head);
        head = next;
    }
}

int main(void) {
    struct node *list = nullptr;
    for (int i = 1; i <= 5; i++) {
        list = push(list, i * i);
    }
    list = reverse(list);
    for (struct node *n = list; n; n = n->next) {
        printf("%d ", n->value);
    }
    printf("\n");
    free_list(list);
    int grid[3][4];
    for (int r = 0; r < 3; r++) {
        for (int c = 0; c < 4; c++) {
            grid[r][c] = r * 10 + c;
        }
    }
    printf("%d %d\n", grid[1][2], grid[2][3]);
    return 0;
}

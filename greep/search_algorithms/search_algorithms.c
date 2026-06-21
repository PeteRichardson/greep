//
//  search_algorithms.c
//  greep
//
//  Registry mapping short algorithm codes to their implementations.
//

#include <string.h>

#include "search_algorithms.h"

typedef struct {
    const char *code;
    const char *name;
    search_alg_t fn;
} algorithm_entry_t;

static const algorithm_entry_t algorithms[] = {
    {"bf",  "brute force",            find_bf},
    {"bmh", "boyer-moore-horspool",   find_bmh},
};

static const int algorithm_count = sizeof(algorithms) / sizeof(algorithms[0]);

search_alg_t find_algorithm(const char *code) {
    for (int i = 0; i < algorithm_count; i++) {
        if (strcmp(algorithms[i].code, code) == 0) {
            return algorithms[i].fn;
        }
    }
    return NULL;
}

void list_algorithms(FILE *out) {
    for (int i = 0; i < algorithm_count; i++) {
        fprintf(out, "%-6s %s\n", algorithms[i].code, algorithms[i].name);
    }
}

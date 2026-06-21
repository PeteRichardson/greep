//
//  filelist.c
//  greep
//
//  Resolves the final file list for a run: reads -f/--filelist files and
//  expands any directory arguments into the regular files they contain.
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <dirent.h>
#include <sys/stat.h>
#include <limits.h>

#include "filelist.h"

#define INITIAL_CAPACITY 16

typedef struct {
    char **items;
    int count;
    int capacity;
} string_list_t;

static void string_list_init(string_list_t *list) {
    list->capacity = INITIAL_CAPACITY;
    list->count = 0;
    list->items = malloc(sizeof(char *) * list->capacity);
}

static void string_list_append(string_list_t *list, const char *str) {
    if (list->count == list->capacity) {
        int new_capacity = list->capacity * 2;
        char **new_items = realloc(list->items, sizeof(char *) * new_capacity);
        if (!new_items) {
            fprintf(stderr, "# ERROR: out of memory while building file list\n");
            exit(EXIT_FAILURE);
        }
        list->items = new_items;
        list->capacity = new_capacity;
    }
    list->items[list->count++] = strdup(str);
}

static int is_hidden(const char *name) {
    return name[0] == '.';
}

static void walk_directory(const char *dir_path, string_list_t *out) {
    DIR *dir = opendir(dir_path);
    if (!dir) {
        fprintf(stderr, "# ERROR: unable to open directory '%s'\n", dir_path);
        return;
    }

    struct dirent *entry;
    while ((entry = readdir(dir)) != NULL) {
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        if (is_hidden(entry->d_name)) {
            continue;
        }

        char child_path[PATH_MAX];
        snprintf(child_path, sizeof(child_path), "%s/%s", dir_path, entry->d_name);

        struct stat sbuf;
        if (stat(child_path, &sbuf) < 0) {
            fprintf(stderr, "# ERROR: stat failed for '%s'\n", child_path);
            continue;
        }

        if (S_ISDIR(sbuf.st_mode)) {
            walk_directory(child_path, out);
        } else if (S_ISREG(sbuf.st_mode)) {
            string_list_append(out, child_path);
        }
    }

    closedir(dir);
}

char **read_filelist(const char *path, int *out_count) {
    FILE *f = fopen(path, "r");
    if (!f) {
        fprintf(stderr, "# ERROR: unable to open filelist '%s'\n", path);
        exit(EXIT_FAILURE);
    }

    string_list_t list;
    string_list_init(&list);

    char line[PATH_MAX];
    while (fgets(line, sizeof(line), f) != NULL) {
        size_t len = strlen(line);
        while (len > 0 && (line[len - 1] == '\n' || line[len - 1] == '\r')) {
            line[--len] = '\0';
        }
        if (len == 0) {
            continue;
        }
        string_list_append(&list, line);
    }

    fclose(f);

    *out_count = list.count;
    return list.items;
}

char **expand_paths(char **paths, int count, int *out_count) {
    string_list_t out;
    string_list_init(&out);

    for (int i = 0; i < count; i++) {
        struct stat sbuf;
        if (stat(paths[i], &sbuf) < 0) {
            // Not statable (e.g. /dev/stdin in some contexts) -- pass through unchanged.
            string_list_append(&out, paths[i]);
            continue;
        }

        if (S_ISDIR(sbuf.st_mode)) {
            walk_directory(paths[i], &out);
        } else {
            string_list_append(&out, paths[i]);
        }
    }

    *out_count = out.count;
    return out.items;
}

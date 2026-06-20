# Boyer-Moore-Horspool Algorithm, Algorithm Selection, Timing, and Filelist Support — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Boyer-Moore-Horspool search algorithm alongside the existing brute-force one, let the user pick which to run via `-a/--algorithm`, add `-t/--timing` instrumentation (per-file + summary), and add `-f/--filelist` plus recursive directory expansion so large, repeatable file sets can be used for performance comparisons.

**Architecture:** A new algorithm registry (`search_algorithms.c`) maps short codes to function pointers implementing the existing `search_alg_t` interface, used by both CLI validation (`options.c`) and `main.c`'s thread dispatch. A new `filelist.c` module resolves the final file list (from positional args or `-f`) and expands any directories into their regular files. `main.c` wraps each thread's `search_alg()` call with `clock_gettime` when `-t` is given and prints per-file and summary timing lines to stderr.

**Tech Stack:** C (gnu11), POSIX (`pthread`, `dirent.h`, `clock_gettime`/`CLOCK_MONOTONIC`, `getopt_long`). No external dependencies, no test framework (none exists in this project — see Verification Approach below).

## Global Constraints

- Target macOS (SDKROOT=macosx, MACOSX_DEPLOYMENT_TARGET=15.0, gnu11), per `CLAUDE.md`.
- No external dependencies — use `getopt_long` from libc, as the project already does.
- Single algorithm per run (no multi-algorithm comparison in one invocation) — out of scope per the spec.
- `--dump-filelist` is explicitly out of scope; leave a `// TODO` comment marking where it would go.

## Verification Approach

This project has no test suite or test framework (confirmed in `CLAUDE.md`: "There is no test target/suite in this project currently"). Each task is verified by building with `make` and running the real `./build/greep` binary against small fixture files created under `/tmp/greep_fixtures/` (not committed to the repo — these are throwaway, recreated by each task's steps). This matches the project's existing conventions (no test infra) while still giving each task a concrete, reproducible pass/fail check.

---

### Task 1: Boyer-Moore-Horspool search algorithm

**Files:**
- Create: `greep/search_algorithms/search_bmh.c`
- Modify: `greep/search_algorithms/search_algorithms.h`
- Modify: `Makefile`

**Interfaces:**
- Produces: `void find_bmh(const char *search_word, const char *filename, const char *start, const unsigned long length, callback_t *found_callback);` — matches the existing `search_alg_t` signature used by `find_bf`.

- [ ] **Step 1: Create the fixture files used to verify this task**

```bash
mkdir -p /tmp/greep_fixtures
cat > /tmp/greep_fixtures/sample.txt <<'EOF'
the quick brown fox
jumps over the lazy dog
needle in a haystack
another line with needle twice needle
no match on this line
needle
EOF
```

- [ ] **Step 2: Write `greep/search_algorithms/search_bmh.c`**

```c
//
//  search_bmh.c
//  greep
//

#include <string.h>

#include "search_algorithms.h"

/*
 Find using the Boyer-Moore-Horspool algorithm.
 Assumes file is in memory. Reports at most one match per line (the line
 containing the first occurrence of search_word), matching the per-line
 reporting behavior of find_bf.
*/
void find_bmh(const char *search_word,
              const char *filename,
              const char *start,
              const unsigned long length,
              callback_t *found_callback) {

    unsigned long word_len = strlen(search_word);
    if (word_len == 0 || length < word_len) {
        return;
    }

    unsigned long shift[256];
    for (int i = 0; i < 256; i++) {
        shift[i] = word_len;
    }
    for (unsigned long i = 0; i < word_len - 1; i++) {
        shift[(unsigned char) search_word[i]] = word_len - 1 - i;
    }

    const char *end = start + length;
    unsigned long lineno = 1;
    const char *line_start = start;
    const char *scan_pos = start;

    const char *window = start;
    while (window + word_len <= end) {
        // Advance line tracking up to the current window position. scan_pos
        // only ever moves forward, so this stays O(n) total across the run
        // even though window can jump ahead by more than one byte per step.
        for (; scan_pos < window; scan_pos++) {
            if (*scan_pos == '\n') {
                lineno++;
                line_start = scan_pos + 1;
            }
        }

        long j = (long) word_len - 1;
        while (j >= 0 && window[j] == search_word[j]) {
            j--;
        }

        if (j < 0) {
            const char *line_end = window + word_len;
            while (line_end < end && *line_end != '\n') {
                line_end++;
            }
            (*found_callback)(filename, lineno, line_start, line_end);

            window = (line_end < end) ? line_end + 1 : end;
            scan_pos = window;
            lineno++;
            line_start = window;
        } else {
            unsigned char bad_char = (unsigned char) window[word_len - 1];
            window += shift[bad_char];
        }
    }
}
```

- [ ] **Step 3: Add the `find_bmh` declaration to `greep/search_algorithms/search_algorithms.h`**

Change:
```c
//  See various algorithm definitions in http://www-igm.univ-mlv.fr/~lecroq/string/index.html
void find_bf(const char *search_word, const char *filename, const char *start, const unsigned long length, callback_t *found_callback);

#endif /* search_algorithms_h */
```
to:
```c
//  See various algorithm definitions in http://www-igm.univ-mlv.fr/~lecroq/string/index.html
void find_bf(const char *search_word, const char *filename, const char *start, const unsigned long length, callback_t *found_callback);
void find_bmh(const char *search_word, const char *filename, const char *start, const unsigned long length, callback_t *found_callback);

#endif /* search_algorithms_h */
```

- [ ] **Step 4: Add `search_bmh.c` to the `Makefile`**

Change:
```makefile
SRCS := $(SRC_DIR)/main.c $(SRC_DIR)/options.c $(SRC_DIR)/search_algorithms/search_default.c
```
to:
```makefile
SRCS := $(SRC_DIR)/main.c $(SRC_DIR)/options.c \
        $(SRC_DIR)/search_algorithms/search_default.c \
        $(SRC_DIR)/search_algorithms/search_bmh.c
```

- [ ] **Step 5: Write a throwaway scratch harness to verify `find_bmh` matches `find_bf`**

```bash
cat > /tmp/greep_fixtures/compare_algs.c <<'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>

#include "search_algorithms.h"

static char results_bf[4096];
static char results_bmh[4096];
static char *cursor;

static void record_callback(const char *filename, unsigned long line_num,
                             const char *line_start, const char *line_end) {
    cursor += sprintf(cursor, "%lu:", line_num);
    for (const char *p = line_start; p < line_end; p++) {
        *cursor++ = *p;
    }
    *cursor++ = '\n';
    *cursor = '\0';
}

int main(int argc, char *argv[]) {
    const char *search_word = argv[1];
    const char *path = argv[2];

    int fd = open(path, O_RDONLY);
    struct stat sbuf;
    fstat(fd, &sbuf);
    void *src = mmap(0, sbuf.st_size, PROT_READ, MAP_SHARED, fd, 0);

    callback_t cb = record_callback;

    cursor = results_bf;
    find_bf(search_word, path, src, sbuf.st_size, &cb);

    cursor = results_bmh;
    find_bmh(search_word, path, src, sbuf.st_size, &cb);

    if (strcmp(results_bf, results_bmh) == 0) {
        printf("PASS: bf and bmh agree for '%s'\n%s", search_word, results_bf);
        return 0;
    } else {
        printf("FAIL: bf and bmh disagree for '%s'\nbf:\n%s\nbmh:\n%s\n",
               search_word, results_bf, results_bmh);
        return 1;
    }
}
EOF
cc -std=gnu11 -Wall -I/Users/pete/.supacode/repos/greep/another_search_algorithm/greep \
   /tmp/greep_fixtures/compare_algs.c \
   /Users/pete/.supacode/repos/greep/another_search_algorithm/greep/search_algorithms/search_default.c \
   /Users/pete/.supacode/repos/greep/another_search_algorithm/greep/search_algorithms/search_bmh.c \
   -o /tmp/greep_fixtures/compare_algs
```

- [ ] **Step 6: Run the scratch harness against several search words and confirm PASS**

```bash
/tmp/greep_fixtures/compare_algs "needle" /tmp/greep_fixtures/sample.txt
/tmp/greep_fixtures/compare_algs "fox" /tmp/greep_fixtures/sample.txt
/tmp/greep_fixtures/compare_algs "line" /tmp/greep_fixtures/sample.txt
/tmp/greep_fixtures/compare_algs "zzz_not_present" /tmp/greep_fixtures/sample.txt
```

Expected: every invocation prints `PASS: ...` and exits 0 (the last one, for a word
that doesn't appear, should print `PASS` with empty output after the word).

- [ ] **Step 7: Build the real project to confirm it still compiles cleanly**

```bash
cd /Users/pete/.supacode/repos/greep/another_search_algorithm && make clean && make
```

Expected: builds with no errors or warnings, produces `build/greep`.

- [ ] **Step 8: Commit**

```bash
git add greep/search_algorithms/search_bmh.c greep/search_algorithms/search_algorithms.h Makefile
git commit -m "feat: add Boyer-Moore-Horspool search algorithm"
```

---

### Task 2: Algorithm registry (`find_algorithm`, `list_algorithms`)

**Files:**
- Create: `greep/search_algorithms/search_algorithms.c`
- Modify: `greep/search_algorithms/search_algorithms.h`
- Modify: `Makefile`

**Interfaces:**
- Consumes: `find_bf`, `find_bmh` (from Task 1, declared in `search_algorithms.h`).
- Produces:
  - `search_alg_t find_algorithm(const char *code);` — returns the matching function pointer or `NULL`.
  - `void list_algorithms(FILE *out);` — prints `"%-6s %s\n"` (code, name) per registered algorithm.

- [ ] **Step 1: Add the new declarations to `greep/search_algorithms/search_algorithms.h`**

Change:
```c
#ifndef search_algorithms_h
#define search_algorithms_h

typedef void (*callback_t) (const char *,  // filename
                            unsigned long,      // line number
                            const char *,       // line start
                            const char *);      // line end
```
to:
```c
#ifndef search_algorithms_h
#define search_algorithms_h

#include <stdio.h>

typedef void (*callback_t) (const char *,  // filename
                            unsigned long,      // line number
                            const char *,       // line start
                            const char *);      // line end
```

Then, after the `find_bmh` declaration added in Task 1, add:
```c
search_alg_t find_algorithm(const char *code);
void list_algorithms(FILE *out);
```

- [ ] **Step 2: Write `greep/search_algorithms/search_algorithms.c`**

```c
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
```

- [ ] **Step 3: Add `search_algorithms.c` to the `Makefile`**

Change:
```makefile
SRCS := $(SRC_DIR)/main.c $(SRC_DIR)/options.c \
        $(SRC_DIR)/search_algorithms/search_default.c \
        $(SRC_DIR)/search_algorithms/search_bmh.c
```
to:
```makefile
SRCS := $(SRC_DIR)/main.c $(SRC_DIR)/options.c \
        $(SRC_DIR)/search_algorithms/search_default.c \
        $(SRC_DIR)/search_algorithms/search_bmh.c \
        $(SRC_DIR)/search_algorithms/search_algorithms.c
```

- [ ] **Step 4: Write a scratch harness to verify the registry**

```bash
cat > /tmp/greep_fixtures/test_registry.c <<'EOF'
#include <stdio.h>
#include <string.h>

#include "search_algorithms.h"

int main(void) {
    int failures = 0;

    if (find_algorithm("bf") != find_bf) {
        printf("FAIL: find_algorithm(\"bf\") did not return find_bf\n");
        failures++;
    }
    if (find_algorithm("bmh") != find_bmh) {
        printf("FAIL: find_algorithm(\"bmh\") did not return find_bmh\n");
        failures++;
    }
    if (find_algorithm("nope") != NULL) {
        printf("FAIL: find_algorithm(\"nope\") should return NULL\n");
        failures++;
    }

    char buf[256];
    FILE *f = fmemopen(buf, sizeof(buf), "w");
    list_algorithms(f);
    fclose(f);
    if (strstr(buf, "bf") == NULL || strstr(buf, "bmh") == NULL) {
        printf("FAIL: list_algorithms output missing expected codes:\n%s\n", buf);
        failures++;
    }

    if (failures == 0) {
        printf("PASS: all registry checks passed\n");
        return 0;
    }
    return 1;
}
EOF
cc -std=gnu11 -Wall -I/Users/pete/.supacode/repos/greep/another_search_algorithm/greep \
   /tmp/greep_fixtures/test_registry.c \
   /Users/pete/.supacode/repos/greep/another_search_algorithm/greep/search_algorithms/search_default.c \
   /Users/pete/.supacode/repos/greep/another_search_algorithm/greep/search_algorithms/search_bmh.c \
   /Users/pete/.supacode/repos/greep/another_search_algorithm/greep/search_algorithms/search_algorithms.c \
   -o /tmp/greep_fixtures/test_registry
```

- [ ] **Step 5: Run the scratch harness and confirm PASS**

```bash
/tmp/greep_fixtures/test_registry
```

Expected: `PASS: all registry checks passed`, exit code 0.

- [ ] **Step 6: Build the real project**

```bash
cd /Users/pete/.supacode/repos/greep/another_search_algorithm && make clean && make
```

Expected: builds with no errors or warnings.

- [ ] **Step 7: Commit**

```bash
git add greep/search_algorithms/search_algorithms.c greep/search_algorithms/search_algorithms.h Makefile
git commit -m "feat: add algorithm registry (find_algorithm, list_algorithms)"
```

---

### Task 3: `-a/--algorithm` and `-l/--list` CLI flags

**Files:**
- Modify: `greep/options.h`
- Modify: `greep/options.c`
- Modify: `greep/main.c`

**Interfaces:**
- Consumes: `find_algorithm`, `list_algorithms` (from Task 2).
- Produces: `arguments_t.algorithm_code` (`const char *`, default `"bf"`), validated by `parse_args` before it returns.

- [ ] **Step 1: Add `algorithm_code` to `greep/options.h`**

Change:
```c
typedef struct arguments_t {
    int  verbose;
    char *search_word;
    char **filenames;
    int  filecount;
} arguments_t;

static char *default_input_files[] = { "/dev/stdin" };

static arguments_t ARGUMENT_DEFAULT_VALUES = {
    .verbose = 0,
    .search_word = NULL,
    .filecount = 1,
    .filenames = (char**)default_input_files
};
```
to:
```c
typedef struct arguments_t {
    int  verbose;
    char *search_word;
    char **filenames;
    int  filecount;
    const char *algorithm_code;
} arguments_t;

static char *default_input_files[] = { "/dev/stdin" };

static arguments_t ARGUMENT_DEFAULT_VALUES = {
    .verbose = 0,
    .search_word = NULL,
    .filecount = 1,
    .filenames = (char**)default_input_files,
    .algorithm_code = "bf"
};
```

- [ ] **Step 2: Rewrite `greep/options.c`**

```c
//
//  options.c
//  greep
//
//  Created by Peter Richardson on 12/11/21.
//

#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>

#include "options.h"
#include "search_algorithms/search_algorithms.h"

static char usage[] =
    "usage: greep [-v] [-a ALGORITHM] STRING [FILES...]\n"
    "       greep -l\n";

static struct option long_options[] = {
    {"verbose",   no_argument,       0, 'v'},
    {"algorithm", required_argument, 0, 'a'},
    {"list",      no_argument,       0, 'l'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "va:l", long_options, NULL)) != -1) {
        switch (opt) {
            case 'v':
                args->verbose = 1;
                break;

            case 'a':
                args->algorithm_code = optarg;
                break;

            case 'l':
                list_algorithms(stdout);
                exit(EXIT_SUCCESS);

            default:
                fputs(usage, stderr);
                exit(EXIT_FAILURE);
        }
    }

    if (find_algorithm(args->algorithm_code) == NULL) {
        fprintf(stderr, "# ERROR: unknown algorithm '%s'. Run 'greep -l' to see available algorithms.\n", args->algorithm_code);
        exit(EXIT_FAILURE);
    }

    if (optind >= argc) {
        fputs(usage, stderr);
        exit(EXIT_FAILURE);
    }

    args->search_word = argv[optind];

    if (optind + 1 < argc) {
        args->filenames = &argv[optind + 1];
        args->filecount = argc - optind - 1;
    }
}
```

- [ ] **Step 3: Update `greep/main.c` to resolve and use the chosen algorithm**

Change:
```c
    pthread_t threads[args.filecount];
    threadargs_t payloads[args.filecount];
    for (int i = 0; i < args.filecount; i++ ) {
        threadargs_t thread_args = {
            .search_word = args.search_word,
            .filename = args.filenames[i],
            .search_alg = find_bf,
            .found_callback = found_callback
        };
        payloads[i] = thread_args;
    }
```
to:
```c
    search_alg_t chosen_alg = find_algorithm(args.algorithm_code);

    pthread_t threads[args.filecount];
    threadargs_t payloads[args.filecount];
    for (int i = 0; i < args.filecount; i++ ) {
        threadargs_t thread_args = {
            .search_word = args.search_word,
            .filename = args.filenames[i],
            .search_alg = chosen_alg,
            .found_callback = found_callback
        };
        payloads[i] = thread_args;
    }
```

- [ ] **Step 4: Build**

```bash
cd /Users/pete/.supacode/repos/greep/another_search_algorithm && make clean && make
```

Expected: builds with no errors or warnings.

- [ ] **Step 5: Manually verify end-to-end CLI behavior**

```bash
./build/greep -l
./build/greep -a bf needle /tmp/greep_fixtures/sample.txt
./build/greep -a bmh needle /tmp/greep_fixtures/sample.txt
./build/greep -a bogus needle /tmp/greep_fixtures/sample.txt; echo "exit=$?"
./build/greep needle /tmp/greep_fixtures/sample.txt
```

Expected:
- `-l` prints `bf     brute force` and `bmh    boyer-moore-horspool` (and exits 0, no `STRING` required).
- `-a bf` and `-a bmh` produce identical match output (lines containing "needle").
- `-a bogus` prints the unknown-algorithm error to stderr and `exit=1`.
- No `-a` flag still defaults to brute force and produces the same matches.

- [ ] **Step 6: Commit**

```bash
git add greep/options.h greep/options.c greep/main.c
git commit -m "feat: add -a/--algorithm and -l/--list CLI flags"
```

---

### Task 4: `-f/--filelist` and recursive directory expansion

**Files:**
- Create: `greep/filelist.h`
- Create: `greep/filelist.c`
- Modify: `greep/options.h`
- Modify: `greep/options.c`
- Modify: `Makefile`

**Interfaces:**
- Produces:
  - `char **read_filelist(const char *path, int *out_count);` — reads newline-delimited filenames from `path` (blank lines skipped), returns a heap-allocated array of heap-allocated strings.
  - `char **expand_paths(char **paths, int count, int *out_count);` — for each entry: if it's a directory, recursively replaces it with the regular files inside it (depth-first, skipping any entry whose basename starts with `.`); otherwise passes it through unchanged. Returns a new heap-allocated array of heap-allocated strings.
- Consumes (in `options.c`): both functions above.

- [ ] **Step 1: Write `greep/filelist.h`**

```c
//
//  filelist.h
//  greep
//

#ifndef filelist_h
#define filelist_h

char **read_filelist(const char *path, int *out_count);
char **expand_paths(char **paths, int count, int *out_count);

#endif /* filelist_h */
```

- [ ] **Step 2: Write `greep/filelist.c`**

```c
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
        list->capacity *= 2;
        list->items = realloc(list->items, sizeof(char *) * list->capacity);
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
```

- [ ] **Step 3: Add `filelist_path` to `greep/options.h`**

Change:
```c
typedef struct arguments_t {
    int  verbose;
    char *search_word;
    char **filenames;
    int  filecount;
    const char *algorithm_code;
} arguments_t;

static char *default_input_files[] = { "/dev/stdin" };

static arguments_t ARGUMENT_DEFAULT_VALUES = {
    .verbose = 0,
    .search_word = NULL,
    .filecount = 1,
    .filenames = (char**)default_input_files,
    .algorithm_code = "bf"
};
```
to:
```c
typedef struct arguments_t {
    int  verbose;
    char *search_word;
    char **filenames;
    int  filecount;
    const char *algorithm_code;
    const char *filelist_path;
} arguments_t;

static char *default_input_files[] = { "/dev/stdin" };

static arguments_t ARGUMENT_DEFAULT_VALUES = {
    .verbose = 0,
    .search_word = NULL,
    .filecount = 1,
    .filenames = (char**)default_input_files,
    .algorithm_code = "bf",
    .filelist_path = NULL
};
```

- [ ] **Step 4: Update `greep/options.c` to parse `-f/--filelist`, validate it against positional args, read it, and expand directories**

Change:
```c
#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>

#include "options.h"
#include "search_algorithms/search_algorithms.h"

static char usage[] =
    "usage: greep [-v] [-a ALGORITHM] STRING [FILES...]\n"
    "       greep -l\n";

static struct option long_options[] = {
    {"verbose",   no_argument,       0, 'v'},
    {"algorithm", required_argument, 0, 'a'},
    {"list",      no_argument,       0, 'l'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "va:l", long_options, NULL)) != -1) {
        switch (opt) {
            case 'v':
                args->verbose = 1;
                break;

            case 'a':
                args->algorithm_code = optarg;
                break;

            case 'l':
                list_algorithms(stdout);
                exit(EXIT_SUCCESS);

            default:
                fputs(usage, stderr);
                exit(EXIT_FAILURE);
        }
    }

    if (find_algorithm(args->algorithm_code) == NULL) {
        fprintf(stderr, "# ERROR: unknown algorithm '%s'. Run 'greep -l' to see available algorithms.\n", args->algorithm_code);
        exit(EXIT_FAILURE);
    }

    if (optind >= argc) {
        fputs(usage, stderr);
        exit(EXIT_FAILURE);
    }

    args->search_word = argv[optind];

    if (optind + 1 < argc) {
        args->filenames = &argv[optind + 1];
        args->filecount = argc - optind - 1;
    }
}
```
to:
```c
#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>

#include "options.h"
#include "filelist.h"
#include "search_algorithms/search_algorithms.h"

static char usage[] =
    "usage: greep [-v] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]\n"
    "       greep -l\n";

static struct option long_options[] = {
    {"verbose",   no_argument,       0, 'v'},
    {"algorithm", required_argument, 0, 'a'},
    {"list",      no_argument,       0, 'l'},
    // TODO: --dump-filelist <path> to capture the resolved file set for reuse
    {"filelist",  required_argument, 0, 'f'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "va:lf:", long_options, NULL)) != -1) {
        switch (opt) {
            case 'v':
                args->verbose = 1;
                break;

            case 'a':
                args->algorithm_code = optarg;
                break;

            case 'l':
                list_algorithms(stdout);
                exit(EXIT_SUCCESS);

            case 'f':
                args->filelist_path = optarg;
                break;

            default:
                fputs(usage, stderr);
                exit(EXIT_FAILURE);
        }
    }

    if (find_algorithm(args->algorithm_code) == NULL) {
        fprintf(stderr, "# ERROR: unknown algorithm '%s'. Run 'greep -l' to see available algorithms.\n", args->algorithm_code);
        exit(EXIT_FAILURE);
    }

    if (optind >= argc) {
        fputs(usage, stderr);
        exit(EXIT_FAILURE);
    }

    args->search_word = argv[optind];

    int positional_filecount = argc - optind - 1;
    if (args->filelist_path != NULL && positional_filecount > 0) {
        fprintf(stderr, "# ERROR: cannot combine -f/--filelist with positional file arguments\n");
        exit(EXIT_FAILURE);
    }

    char **initial_filenames;
    int initial_filecount;

    if (args->filelist_path != NULL) {
        initial_filenames = read_filelist(args->filelist_path, &initial_filecount);
    } else if (positional_filecount > 0) {
        initial_filenames = &argv[optind + 1];
        initial_filecount = positional_filecount;
    } else {
        initial_filenames = args->filenames;
        initial_filecount = args->filecount;
    }

    args->filenames = expand_paths(initial_filenames, initial_filecount, &args->filecount);
}
```

- [ ] **Step 5: Add `filelist.c` to the `Makefile`**

Change:
```makefile
SRCS := $(SRC_DIR)/main.c $(SRC_DIR)/options.c \
        $(SRC_DIR)/search_algorithms/search_default.c \
        $(SRC_DIR)/search_algorithms/search_bmh.c \
        $(SRC_DIR)/search_algorithms/search_algorithms.c
```
to:
```makefile
SRCS := $(SRC_DIR)/main.c $(SRC_DIR)/options.c $(SRC_DIR)/filelist.c \
        $(SRC_DIR)/search_algorithms/search_default.c \
        $(SRC_DIR)/search_algorithms/search_bmh.c \
        $(SRC_DIR)/search_algorithms/search_algorithms.c
```

- [ ] **Step 6: Build**

```bash
cd /Users/pete/.supacode/repos/greep/another_search_algorithm && make clean && make
```

Expected: builds with no errors or warnings.

- [ ] **Step 7: Create fixture directory tree and filelist, then verify behavior manually**

```bash
mkdir -p /tmp/greep_fixtures/tree/sub/.hiddensub
mkdir -p /tmp/greep_fixtures/tree/.hidden
echo "needle in tree top" > /tmp/greep_fixtures/tree/top.txt
echo "needle in tree sub" > /tmp/greep_fixtures/tree/sub/sub.txt
echo "needle in hidden sub, should be skipped" > /tmp/greep_fixtures/tree/sub/.hiddensub/skip.txt
echo "needle in hidden dir, should be skipped" > /tmp/greep_fixtures/tree/.hidden/skip.txt
echo ".hidden_top_file, should be skipped" > /tmp/greep_fixtures/tree/.hiddentop.txt

printf '/tmp/greep_fixtures/sample.txt\n\n/tmp/greep_fixtures/tree/top.txt\n' > /tmp/greep_fixtures/list.txt

./build/greep needle /tmp/greep_fixtures/tree
./build/greep -f /tmp/greep_fixtures/list.txt needle
./build/greep -f /tmp/greep_fixtures/list.txt needle /tmp/greep_fixtures/sample.txt; echo "exit=$?"
```

Expected:
- Directory run finds matches in `top.txt` and `sub/sub.txt` only — nothing from `.hidden/` or `.hiddensub/` or `.hiddentop.txt`.
- `-f` run reads the two listed files (blank line skipped) and finds matches in both.
- Combining `-f` with a positional file prints the conflict error to stderr and `exit=1`.

- [ ] **Step 8: Commit**

```bash
git add greep/filelist.h greep/filelist.c greep/options.h greep/options.c Makefile
git commit -m "feat: add -f/--filelist and recursive directory expansion"
```

---

### Task 5: `-t/--timing` instrumentation

**Files:**
- Modify: `greep/options.h`
- Modify: `greep/options.c`
- Modify: `greep/main.c`

**Interfaces:**
- Produces: `arguments_t.timing` (`int`, default 0).
- `main.c` prints, to stderr, when `args.timing` is set:
  - `#TIMING %8ld %s\n` (elapsed microseconds, filename) — once per file, printed by that file's thread as it finishes.
  - `#COMMAND <full reconstructed argv>` and `#TIMING_SUMMARY algorithm=<code> files=<n> bytes=<total> min=<usec> avg=<usec> max=<usec>` — once, after all threads join.

- [ ] **Step 1: Add `timing` to `greep/options.h`**

Change:
```c
typedef struct arguments_t {
    int  verbose;
    char *search_word;
    char **filenames;
    int  filecount;
    const char *algorithm_code;
    const char *filelist_path;
} arguments_t;

static char *default_input_files[] = { "/dev/stdin" };

static arguments_t ARGUMENT_DEFAULT_VALUES = {
    .verbose = 0,
    .search_word = NULL,
    .filecount = 1,
    .filenames = (char**)default_input_files,
    .algorithm_code = "bf",
    .filelist_path = NULL
};
```
to:
```c
typedef struct arguments_t {
    int  verbose;
    int  timing;
    char *search_word;
    char **filenames;
    int  filecount;
    const char *algorithm_code;
    const char *filelist_path;
} arguments_t;

static char *default_input_files[] = { "/dev/stdin" };

static arguments_t ARGUMENT_DEFAULT_VALUES = {
    .verbose = 0,
    .timing = 0,
    .search_word = NULL,
    .filecount = 1,
    .filenames = (char**)default_input_files,
    .algorithm_code = "bf",
    .filelist_path = NULL
};
```

- [ ] **Step 2: Add `-t/--timing` parsing to `greep/options.c`**

Change:
```c
static char usage[] =
    "usage: greep [-v] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]\n"
    "       greep -l\n";

static struct option long_options[] = {
    {"verbose",   no_argument,       0, 'v'},
    {"algorithm", required_argument, 0, 'a'},
    {"list",      no_argument,       0, 'l'},
    // TODO: --dump-filelist <path> to capture the resolved file set for reuse
    {"filelist",  required_argument, 0, 'f'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "va:lf:", long_options, NULL)) != -1) {
        switch (opt) {
            case 'v':
                args->verbose = 1;
                break;

            case 'a':
                args->algorithm_code = optarg;
                break;

            case 'l':
                list_algorithms(stdout);
                exit(EXIT_SUCCESS);

            case 'f':
                args->filelist_path = optarg;
                break;

            default:
                fputs(usage, stderr);
                exit(EXIT_FAILURE);
        }
    }
```
to:
```c
static char usage[] =
    "usage: greep [-v] [-t] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]\n"
    "       greep -l\n";

static struct option long_options[] = {
    {"verbose",   no_argument,       0, 'v'},
    {"timing",    no_argument,       0, 't'},
    {"algorithm", required_argument, 0, 'a'},
    {"list",      no_argument,       0, 'l'},
    // TODO: --dump-filelist <path> to capture the resolved file set for reuse
    {"filelist",  required_argument, 0, 'f'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "vta:lf:", long_options, NULL)) != -1) {
        switch (opt) {
            case 'v':
                args->verbose = 1;
                break;

            case 't':
                args->timing = 1;
                break;

            case 'a':
                args->algorithm_code = optarg;
                break;

            case 'l':
                list_algorithms(stdout);
                exit(EXIT_SUCCESS);

            case 'f':
                args->filelist_path = optarg;
                break;

            default:
                fputs(usage, stderr);
                exit(EXIT_FAILURE);
        }
    }
```

- [ ] **Step 3: Rewrite `greep/main.c`**

```c
//
//  main.c
//  greep
//
//  Created by Peter Richardson on 12/3/21.
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <time.h>

#include "search_algorithms/search_algorithms.h"
#include "options.h"

typedef struct threadargs_t {
    const char *search_word;
    const char *filename;
    search_alg_t search_alg;
    callback_t found_callback;
    int timing;
    long *elapsed_usec_out;
    unsigned long *bytes_out;
} threadargs_t;

// Read all of fd into a growable heap buffer. Used for pipes/ttys, which
// can't be mmap'd and may block waiting for data (e.g. interactive stdin).
static void *slurp_fd(int fd, size_t *out_len) {
    size_t cap = 1 << 16;
    size_t len = 0;
    char *buf = malloc(cap);
    if (!buf) return NULL;

    ssize_t n;
    while ((n = read(fd, buf + len, cap - len)) > 0) {
        len += (size_t) n;
        if (len == cap) {
            cap *= 2;
            char *grown = realloc(buf, cap);
            if (!grown) {
                free(buf);
                return NULL;
            }
            buf = grown;
        }
    }
    *out_len = len;
    return buf;
}

void *threaded_find(void *arg) {
    threadargs_t thread_args = *(threadargs_t *) arg;

    int fdin;
    struct stat sbuf;
    void *src;
    int is_mmapped = 0;
    unsigned long src_len;

    if ((fdin = open(thread_args.filename, O_RDONLY)) < 0) {
        fprintf(stderr, "# ERROR: Unable to open file '%s' for reading\n", thread_args.filename);
        return (void*) 0;
    }
    if (fstat(fdin, &sbuf) < 0) {
        fprintf(stderr, "# ERROR: fstat error reading '%s'\n", thread_args.filename);
        close(fdin);
        return (void*) 0;
    }

    if (S_ISREG(sbuf.st_mode)) {
        // Regular file: mmap it.
        if ((src = mmap(0, sbuf.st_size, PROT_READ, MAP_SHARED, fdin, 0)) == MAP_FAILED) {
            fprintf(stderr, "# ERROR: mmap error for '%s'\n", thread_args.filename);
            close(fdin);
            return (void*) 0;
        }
        is_mmapped = 1;
        src_len = sbuf.st_size;
    } else {
        // Pipe, FIFO, or TTY (e.g. /dev/stdin with no redirection): can't be
        // mmap'd, so read it instead. This blocks until EOF, matching the
        // behavior of standard CLI tools when waiting on interactive stdin.
        size_t len = 0;
        src = slurp_fd(fdin, &len);
        if (!src) {
            fprintf(stderr, "# ERROR: out of memory reading '%s'\n", thread_args.filename);
            close(fdin);
            return (void*) 0;
        }
        src_len = (unsigned long) len;
    }

    *thread_args.bytes_out = src_len;

    // ACTUALLY DO THE WORK.   Call the specified search algorithm.
    if (thread_args.timing) {
        struct timespec t0, t1;
        clock_gettime(CLOCK_MONOTONIC, &t0);
        (thread_args.search_alg)(thread_args.search_word, thread_args.filename, src, src_len, &thread_args.found_callback);
        clock_gettime(CLOCK_MONOTONIC, &t1);

        long elapsed_usec = (t1.tv_sec - t0.tv_sec) * 1000000L + (t1.tv_nsec - t0.tv_nsec) / 1000L;
        *thread_args.elapsed_usec_out = elapsed_usec;
        fprintf(stderr, "#TIMING %8ld %s\n", elapsed_usec, thread_args.filename);
    } else {
        (thread_args.search_alg)(thread_args.search_word, thread_args.filename, src, src_len, &thread_args.found_callback);
    }

    if (is_mmapped) {
        munmap(src, src_len);
    } else {
        free(src);
    }
    close(fdin);
    return 0;
}

// Run this code whenever a match is found
void found_callback(const char *filename, unsigned long line_num, const char *line_start, const char *line_end) {
    printf("%s:%lu ", filename, line_num);
    for ( char *p = (char*) line_start; p < line_end; p++) {
        putchar(*p);
    }
    putchar('\n');
}

int main(int argc, char *argv[]) {
    arguments_t args = ARGUMENT_DEFAULT_VALUES;
    parse_args(argc, argv, &args);

    search_alg_t chosen_alg = find_algorithm(args.algorithm_code);

    if (args.verbose) {
        fprintf(stderr, "# Searching for '%s'\n", args.search_word);
    }

    pthread_t threads[args.filecount];
    threadargs_t payloads[args.filecount];
    long elapsed_usec[args.filecount];
    unsigned long bytes_searched[args.filecount];

    for (int i = 0; i < args.filecount; i++ ) {
        elapsed_usec[i] = 0;
        bytes_searched[i] = 0;
        threadargs_t thread_args = {
            .search_word = args.search_word,
            .filename = args.filenames[i],
            .search_alg = chosen_alg,
            .found_callback = found_callback,
            .timing = args.timing,
            .elapsed_usec_out = &elapsed_usec[i],
            .bytes_out = &bytes_searched[i]
        };
        payloads[i] = thread_args;
    }

    for (int i = 0; i < args.filecount; i++ ) {
        if (args.verbose) {
            fprintf(stderr, "# Processing file %d: %s\n", i, args.filenames[i]);
        }
        int err = pthread_create(&threads[i], NULL, &threaded_find, &payloads[i]);
        if (err) {
            printf("%s\n", strerror(err), stderr);
            exit(EXIT_FAILURE);
        }
    }

    for (int i=0; i<args.filecount; i++) {
        pthread_join(threads[i], NULL);
    }

    if (args.timing) {
        fprintf(stderr, "#COMMAND");
        for (int i = 0; i < argc; i++) {
            fprintf(stderr, " %s", argv[i]);
        }
        fprintf(stderr, "\n");

        long min = elapsed_usec[0];
        long max = elapsed_usec[0];
        long total = 0;
        unsigned long total_bytes = 0;
        for (int i = 0; i < args.filecount; i++) {
            if (elapsed_usec[i] < min) min = elapsed_usec[i];
            if (elapsed_usec[i] > max) max = elapsed_usec[i];
            total += elapsed_usec[i];
            total_bytes += bytes_searched[i];
        }
        long avg = total / args.filecount;

        fprintf(stderr, "#TIMING_SUMMARY algorithm=%s files=%d bytes=%lu min=%ld avg=%ld max=%ld\n",
                args.algorithm_code, args.filecount, total_bytes, min, avg, max);
    }

    exit(EXIT_SUCCESS);
}
```

(Note: this preserves the pre-existing `printf("%s\n", strerror(err), stderr);` line as-is — it has a latent bug (extra argument, wrong stream) but it's unrelated to this change and out of scope.)

- [ ] **Step 4: Build**

```bash
cd /Users/pete/.supacode/repos/greep/another_search_algorithm && make clean && make
```

Expected: builds with no errors or warnings.

- [ ] **Step 5: Manually verify timing output**

```bash
./build/greep -t needle /tmp/greep_fixtures/sample.txt /tmp/greep_fixtures/tree/top.txt /tmp/greep_fixtures/tree/sub/sub.txt
echo "---"
./build/greep -t -a bmh needle /tmp/greep_fixtures/sample.txt /tmp/greep_fixtures/tree/top.txt /tmp/greep_fixtures/tree/sub/sub.txt
echo "---"
./build/greep -v -t needle /tmp/greep_fixtures/sample.txt
echo "---"
./build/greep needle /tmp/greep_fixtures/sample.txt
```

Expected:
- First two runs: one `#TIMING <usec> <filename>` line per file on stderr, then `#COMMAND ...` showing the full invocation, then one `#TIMING_SUMMARY algorithm=<bf|bmh> files=3 bytes=<n> min=<n> avg=<n> max=<n>` line, with `min <= avg <= max`.
- Third run (`-v -t` together): both the existing `# Searching for...` / `# Processing file...` lines AND the `#TIMING`/`#COMMAND`/`#TIMING_SUMMARY` lines appear, without interfering with each other.
- Fourth run (neither flag): no `#TIMING` or `#COMMAND` lines at all — only match output.

- [ ] **Step 6: Commit**

```bash
git add greep/options.h greep/options.c greep/main.c
git commit -m "feat: add -t/--timing instrumentation with per-file and summary output"
```

---

### Task 6: Update usage text and `CLAUDE.md`

**Files:**
- Modify: `greep/options.c` (usage string — already updated incrementally in Tasks 3–5; this task verifies it's complete and accurate)
- Modify: `CLAUDE.md`

**Interfaces:** None (documentation only).

- [ ] **Step 1: Confirm the final usage string in `greep/options.c` lists every flag**

It should read exactly:
```c
static char usage[] =
    "usage: greep [-v] [-t] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]\n"
    "       greep -l\n";
```
This was already set in Task 5, Step 2 — no further code change needed here, just confirm by reading the file.

- [ ] **Step 2: Update the "Running" and "Architecture" sections of `CLAUDE.md`**

Change:
```markdown
## Running

```
./build/greep STRING [FILES...]
```

If no files are given, defaults to reading from `/dev/stdin`. `-v`/`--verbose` prints
progress to stderr.
```
to:
```markdown
## Running

```
./build/greep [-v] [-t] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]
greep -l
```

If no files are given, defaults to reading from `/dev/stdin`. If a given file path is
a directory, it is searched recursively (dotfiles and dotdirs are skipped).

- `-v`/`--verbose` prints progress (which files are being processed) to stderr.
- `-t`/`--timing` prints per-file timing (`#TIMING <microseconds> <filename>`) to
  stderr as each file finishes, then a `#COMMAND` line (the full invocation) and a
  `#TIMING_SUMMARY` line (min/avg/max microseconds, file count, total bytes) after
  all files are done. Independent of `-v` — both can be combined.
- `-a`/`--algorithm CODE` selects the search algorithm (default `bf`). Run `greep -l`
  to list available codes.
- `-l`/`--list` prints the available algorithm codes and exits.
- `-f`/`--filelist PATH` reads the file list (one path per line) from `PATH` instead
  of positional file arguments. Cannot be combined with positional file arguments.
  A single run always uses one algorithm for all files.
```

And update the architecture bullet about `search_algorithms/`:
```markdown
- `greep/search_algorithms/` — pluggable string-search backends. Each algorithm has the
  signature `search_alg_t` (`search_algorithms.h`): given a search word and an mmap'd
  buffer, it scans for matches and invokes a `callback_t` per match with the file name,
  line number, and pointers to the start/end of the matching line.
  - `search_default.c` implements `find_bf`, a brute-force line-scanning search. This is
    the only algorithm wired up in `main.c` today; the architecture anticipates more
    being added under this directory and selected via `search_alg_t` function pointers.
```
to:
```markdown
- `greep/search_algorithms/` — pluggable string-search backends. Each algorithm has the
  signature `search_alg_t` (`search_algorithms.h`): given a search word and an mmap'd
  buffer, it scans for matches and invokes a `callback_t` per match with the file name,
  line number, and pointers to the start/end of the matching line.
  - `search_default.c` implements `find_bf`, a brute-force line-scanning search.
  - `search_bmh.c` implements `find_bmh`, a Boyer-Moore-Horspool search.
  - `search_algorithms.c` is a small registry (`find_algorithm`/`list_algorithms`)
    mapping `-a` codes (`bf`, `bmh`) to these implementations. A single run uses one
    algorithm for all files; `main.c` resolves it once via `find_algorithm` before
    spawning threads.
- `greep/filelist.c`/`filelist.h` — resolves the final file list for a run: reads
  `-f/--filelist` files (`read_filelist`) and recursively expands any directory
  arguments into their regular files, skipping dotfiles/dotdirs (`expand_paths`).
```

- [ ] **Step 3: Build one final time to confirm everything still compiles**

```bash
cd /Users/pete/.supacode/repos/greep/another_search_algorithm && make clean && make
```

Expected: builds with no errors or warnings.

- [ ] **Step 4: Clean up scratch fixtures (not part of the repo)**

```bash
rm -rf /tmp/greep_fixtures
```

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document -a/-l/-t/-f flags and directory expansion in CLAUDE.md"
```

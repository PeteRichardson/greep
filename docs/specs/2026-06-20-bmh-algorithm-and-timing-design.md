# Design: Boyer-Moore-Horspool algorithm, algorithm selection, timing, and filelist support

Date: 2026-06-20

## Goal

Add a second, more efficient string-search algorithm to greep, let the user choose
between algorithms at runtime, and add instrumentation to measure and compare
per-file and aggregate search performance across algorithms. Also make it practical
to run the same, large file set repeatably across multiple algorithm runs.

This is the first step toward a longer-term goal of evaluating and comparing the
performance of multiple search algorithms against various test data sets.

## 1. New algorithm: Boyer-Moore-Horspool

- New file `greep/search_algorithms/search_bmh.c` implementing:

  ```c
  void find_bmh(const char *search_word, const char *filename,
                const char *start, const unsigned long length,
                callback_t *found_callback);
  ```

  matching the existing `search_alg_t` signature used by `find_bf`.
- Builds a 256-entry bad-character shift table from `search_word` once, then scans
  the buffer using the standard Horspool skip logic. On a full match, finds the line
  bounds (same approach as `find_bf`: scan back to start-of-line / forward to
  end-of-line) and invokes `found_callback`.
- Declared in `search_algorithms.h` alongside `find_bf`.

## 2. Algorithm registry + CLI selection

New file `greep/search_algorithms/search_algorithms.c` (registry implementation,
separate from the `search_algorithms.h` interface declarations) containing a static
table:

```c
typedef struct {
    const char *code;     // e.g. "bf"
    const char *name;     // e.g. "brute force"
    search_alg_t fn;
} algorithm_entry_t;

static const algorithm_entry_t algorithms[] = {
    {"bf",  "brute force",             find_bf},
    {"bmh", "boyer-moore-horspool",    find_bmh},
};
```

with:

- `search_alg_t find_algorithm(const char *code);` — returns the matching function
  pointer, or `NULL` if `code` isn't found.
- `void list_algorithms(FILE *out);` — prints the code/name table, one per line.

### CLI flags (`options.c` / `options.h`)

- `-a CODE`, `--algorithm CODE` — selects the algorithm by short code. Default
  `"bf"`. If `CODE` isn't in the registry, print usage and exit with failure.
- `-l`, `--list` — prints the algorithm table (via `list_algorithms`) to stdout and
  exits 0 immediately. Does not require `STRING` or file arguments.

`arguments_t` gains `const char *algorithm_code`, defaulting to `"bf"`.
`main.c` resolves it to a function pointer once via `find_algorithm()` before
spawning any threads, and uses that pointer for every thread (single algorithm per
run, per existing scope).

## 3. Filelist support and directory expansion

### `-f`/`--filelist PATH`

- New file `greep/filelist.c` / `filelist.h` providing:
  - `char **read_filelist(const char *path, int *out_count);` — reads `path`,
    one filename per line (blank lines ignored), returns a heap-allocated array of
    heap-allocated strings.
  - `char **expand_paths(char **paths, int count, int *out_count);` — walks the
    given paths; any path that is a directory is recursively expanded into the
    regular files it contains (depth-first, all subdirectories), skipping any file
    or directory whose basename starts with `.`. Plain file paths pass through
    unchanged. Returns a new heap-allocated array of heap-allocated strings (the
    final, fully-expanded file list).
- `arguments_t` gains `const char *filelist_path` (default `NULL`).
- In `options.c`, after parsing flags:
  - If `-f` is given and positional file arguments are also given, that's an
    error — print usage and exit with failure (ambiguous which file set to use).
  - If `-f` is given, call `read_filelist()` to get the initial path list;
    otherwise use the positional args (or the default `/dev/stdin`) as before.
  - Call `expand_paths()` on the initial path list to produce the final
    `args->filenames`/`args->filecount` used by `main.c`. (`/dev/stdin` is a
    character device, not a directory, so it passes through `expand_paths`
    unchanged.)
- Leave a `// TODO: --dump-filelist <path> to capture resolved file set for reuse`
  comment near the `-f` parsing code as a marker for a future addition. Not
  implemented now.

## 4. Timing (`-t`/`--timing`)

- `arguments_t` gains `int timing` (default 0), flag `-t`/`--timing`.
- `threadargs_t` (in `main.c`) gains:
  - `int timing` — copy of `args.timing`, passed to each thread.
  - `long *elapsed_usec_out` — pointer into a `long elapsed_usec[args.filecount]`
    array owned by `main()` (NOT the thread's local stack copy), so the value
    written by the thread is visible to `main()` after `pthread_join`.
- Inside `threaded_find`, when `timing` is set:
  - Record `clock_gettime(CLOCK_MONOTONIC, ...)` immediately before and after the
    `search_alg()` call only (excludes file open/mmap/read/close overhead).
  - Compute elapsed microseconds, write to `*elapsed_usec_out`.
  - Immediately print to stderr: `#TIMING %8ld %s\n` (elapsed microseconds,
    fixed-width right-aligned; filename) — printed in thread-completion order, not
    argument order.
- After all threads are joined, if `timing` is set, `main()`:
  - Prints `#COMMAND ` followed by the full reconstructed command line (`argv[0..argc-1]`
    joined with spaces, as received — i.e. post shell-expansion).
  - Computes `min`, `avg` (integer average of `elapsed_usec[]`, truncated), and
    `max` over `elapsed_usec[]`, and the total bytes searched (sum of each file's
    `src_len`, accumulated the same way as `elapsed_usec` via a shared array written
    by each thread).
  - Prints one summary line to stderr:
    ```
    #TIMING_SUMMARY algorithm=<code> files=<n> bytes=<total> min=<usec> avg=<usec> max=<usec>
    ```

Timing output (`-t`) is independent of `-v`/`--verbose`, which continues to only
print the existing pre-execution progress messages ("Searching for...",
"Processing file N: filename"). The two flags can be combined freely; their output
lines don't overlap or interfere with each other.

## Out of scope (for this change)

- Running multiple algorithms in a single invocation / comparison tables across
  algorithms. (Use separate runs with `-f` against the same filelist for now.)
- `--dump-filelist` (writing the resolved file list back out). Left as a TODO.
- Any algorithm other than `bf` and `bmh`.

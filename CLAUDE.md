# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

greep — a homegrown, simplified grep written in C. Uses memory-mapped files and one
pthread per input file for parallel searching. Targets macOS (SDKROOT=macosx,
MACOSX_DEPLOYMENT_TARGET=15.0, gnu11).

## Build

```
make            # builds build/greep
make clean
```

No external dependencies — CLI parsing uses `getopt_long` from macOS's libc.

There is no test target/suite in this project currently.

## Running

```
./build/greep STRING [FILES...]
```

If no files are given, defaults to reading from `/dev/stdin`. `-v`/`--verbose` prints
progress to stderr.

## Architecture

- `greep/main.c` — entry point. Parses args via `parse_args` (`options.h`/`options.c`), then
  for each input file: spawns a thread (`threaded_find`) that mmaps the file read-only and
  invokes the configured search algorithm against the mapped buffer. Matches are reported
  through a callback (`found_callback`) that prints `filename:lineno <matched line>`.
- `greep/options.c` / `options.h` — `getopt_long`-based CLI parsing. `arguments_t` holds
  parsed state (search word, verbose flag, file list). Default file list is `/dev/stdin`.
- `greep/search_algorithms/` — pluggable string-search backends. Each algorithm has the
  signature `search_alg_t` (`search_algorithms.h`): given a search word and an mmap'd
  buffer, it scans for matches and invokes a `callback_t` per match with the file name,
  line number, and pointers to the start/end of the matching line.
  - `search_default.c` implements `find_bf`, a brute-force line-scanning search. This is
    the only algorithm wired up in `main.c` today; the architecture anticipates more
    being added under this directory and selected via `search_alg_t` function pointers.

The threading model in `main.c` creates and joins one thread per file with no upper
bound on concurrency — if you need to search large file sets, an algorithm or threading
change there is the relevant place to look.

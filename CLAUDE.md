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
  - `search_default.c` implements `find_bf`, a brute-force line-scanning search.
  - `search_bmh.c` implements `find_bmh`, a Boyer-Moore-Horspool search.
  - `search_algorithms.c` is a small registry (`find_algorithm`/`list_algorithms`)
    mapping `-a` codes (`bf`, `bmh`) to these implementations. A single run uses one
    algorithm for all files; `main.c` resolves it once via `find_algorithm` before
    spawning threads.
- `greep/filelist.c`/`filelist.h` — resolves the final file list for a run: reads
  `-f/--filelist` files (`read_filelist`) and recursively expands any directory
  arguments into their regular files, skipping dotfiles/dotdirs (`expand_paths`).

The threading model in `main.c` creates and joins one thread per file with no upper
bound on concurrency — if you need to search large file sets, an algorithm or threading
change there is the relevant place to look.

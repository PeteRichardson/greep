# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

greep — a homegrown, simplified grep written in Rust. Spawns one thread per
input file for parallel searching; files under 1GB are read into memory,
files at or above 1GB are memory-mapped.

## Build

```
cargo build --release   # builds target/release/greep
cargo test               # runs unit + integration tests
```

## Running

```
cargo run -- [-v] [-t] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]
cargo run -- -l
```

If no files are given, defaults to reading from `/dev/stdin`. If a given file path is
a directory, it is searched recursively (dotfiles and dotdirs are skipped).

- `-v`/`--verbose` prints progress (which files are being processed) to stderr.
- `-t`/`--timing` prints per-file timing (`#TIMING <microseconds> <filename>`) to
  stderr as each file finishes, then a `#COMMAND` line (the full invocation) and a
  `#TIMING_SUMMARY` line (min/avg/max microseconds, file count, total bytes) after
  all files are done. Independent of `-v` — both can be combined.
- `-a`/`--algorithm CODE` selects the search algorithm (default `bf`). Run
  `cargo run -- -l` to list available codes.
- `-l`/`--list` prints the available algorithm codes and exits.
- `-f`/`--filelist PATH` reads the file list (one path per line) from `PATH` instead
  of positional file arguments. Cannot be combined with positional file arguments.
  A single run always uses one algorithm for all files.

## Architecture

- `src/main.rs` — entry point: orchestration, thread spawn/join, printing, `-t`
  timing summary.
- `src/options.rs` — `clap`-derived `Args`, `AppError`, and `resolve()` which
  validates the algorithm code, resolves the final file list (filelist vs.
  positional vs. default stdin), and expands directories.
- `src/filelist.rs` — `read_filelist`, `expand_paths` (recursive directory walk,
  skipping dotfiles/dotdirs).
- `src/loader.rs` — `load()`: reads files under 1GB into memory, memory-maps
  files at or above 1GB via `memmap2`.
- `src/search/` — `SearchAlgorithm` trait + registry (`find_algorithm`,
  `list_algorithms`). `brute_force.rs` (`Bf`) and `horspool.rs` (`Bmh`) each
  report at most one match per line.

The threading model in `main.rs` spawns and joins one thread per file with no
upper bound on concurrency.

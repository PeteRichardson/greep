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

To run from a source checkout without installing:

```
cargo run -- [-v] [-t] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]
cargo run -- -l
```

**[README.md](README.md#usage) is the single source of truth for user-facing
behavior** — every flag, the argument defaults, algorithm codes, the 0/1/2 exit
status, worked examples, and known limitations. Do not restate any of it here;
a second copy is exactly what drifts.

When a change alters observable behavior, update the README, not this file.

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

## Project

greep — a homegrown, simplified grep written in Rust. Searches files in
parallel on a fixed pool of worker threads; files under 1GB are read into memory,
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

- `src/main.rs` — entry point: orchestration, wiring the walk to the pool,
  printing, `-t` timing summary.
- `src/pool.rs` — `run_ordered()`: a fixed pool of worker threads fed lazily
  from an iterator, results handed back in job order through a bounded window.
- `src/options.rs` — `clap`-derived `Args`, `AppError`, and `resolve()` which
  validates the algorithm code and resolves the paths to search (filelist vs.
  positional vs. default stdin). Directories are not expanded here.
- `src/filelist.rs` — `read_filelist`, `walk` (lazy depth-first directory walk,
  skipping dotfiles/dotdirs; an unreadable directory is a `WalkItem` in sequence).
- `src/loader.rs` — `load()`: reads files under 1GB into memory, memory-maps
  files at or above 1GB via `memmap2`.
- `src/search/` — `SearchAlgorithm` trait + registry (`find_algorithm`,
  `list_algorithms`). `brute_force.rs` (`Bf`) and `horspool.rs` (`Bmh`) each
  report at most one match per line.

The thread count is `available_parallelism()`, whatever the file count. The
walk runs on the main thread, interleaved with emitting results, so it overlaps
the search instead of preceding it.

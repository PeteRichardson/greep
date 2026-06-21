# Design: Rust rewrite of greep (core port)

Date: 2026-06-21

## Goal

Rewrite greep (currently C, threaded, mmap-based) in Rust, replacing the C
implementation on this branch entirely. This covers feature parity with the
current C tool plus the read-vs-mmap loading strategy decided alongside this
design (read files under 1GB, mmap files at or above 1GB). Two related
follow-on ideas were raised in discussion but are explicitly out of scope for
this design and will get their own brainstorm/plan cycles later:

- A line-buffered streaming "watch" mode for stdin (genuinely unbounded
  input, e.g. `tail -f | greep`), since the current read-to-EOF approach
  cannot support that regardless of language.
- A third search algorithm (Quick Search / Sunday's algorithm), which was
  identified as a good next step for algorithm coverage but is independent
  of the language rewrite.

This is a from-scratch rewrite, not a line-by-line port: the goal is
behavioral parity with the C tool's CLI surface and output data, using
idiomatic Rust patterns (see Dependencies and Architecture below) rather than
replicating C's pointer/threading patterns where Rust offers a clearly better
fit.

## Scope: feature parity target

Everything the C tool (as of the `another_search_algorithm` branch, merged to
main) does:

- Two search algorithms: brute force (`bf`) and Boyer-Moore-Horspool (`bmh`),
  selectable via `-a/--algorithm CODE` (default `bf`).
- `-l/--list` to list available algorithm codes and exit.
- `-v/--verbose` progress messages.
- `-t/--timing` per-file and summary timing output.
- `-f/--filelist PATH` to read the file list from a file instead of
  positional arguments (error if combined with positional file arguments).
- Recursive directory expansion: any file argument that is a directory is
  walked recursively, skipping dotfiles/dotdirs at every depth.
- Default input is `/dev/stdin` when no files are given.
- One thread per input file, each reporting matches as `filename:lineno line`.

New in this rewrite: files are read into memory (not mmap'd) when under 1GB,
and mmap'd at or above 1GB — this is a behavior change from the C version
(which always mmap'd regular files), motivated by two things discussed
during design: (1) for files small enough to comfortably buffer, `read()`
avoids mixing page-fault latency into the `-t` timing measurement, since
`mmap()` only sets up the mapping and actual page-ins happen on first touch
*during* the timed search call; (2) `mmap` remains the better choice for
very large files since the kernel can reclaim clean, file-backed mmap'd
pages under memory pressure without needing swap, unlike a `Vec<u8>` holding
the whole file.

## Dependencies

Unlike the C version's "no external dependencies" stance, this rewrite uses
a small set of well-established crates rather than hand-rolling everything
in `std`:

- `clap` (derive API) for CLI argument parsing.
- `memmap2` for memory-mapping files at or above the 1GB threshold.
- `thiserror` for the fatal top-level error enum.
- `assert_cmd` (dev-dependency only) for integration tests driving the built
  binary.

## Architecture

Single binary crate. `Cargo.toml` and `src/` live at the repo root, replacing
the C sources and `Makefile` entirely on this branch.

```
src/
  main.rs              entry point: orchestration, thread spawn/join, printing
  options.rs           clap Args struct, -l early-exit handling
  filelist.rs           read_filelist, expand_paths (recursive dir walk)
  loader.rs            read()-vs-mmap file loading by size threshold
  search/
    mod.rs             SearchAlgorithm trait + registry (find_algorithm, list_algorithms)
    brute_force.rs     Bf: SearchAlgorithm
    horspool.rs        Bmh: SearchAlgorithm
```

### Core types

```rust
pub struct Match {
    pub line_number: u64,
    pub line: String,
}

pub trait SearchAlgorithm {
    fn search(&self, word: &str, buf: &[u8]) -> Vec<Match>;
}
```

Each algorithm collects and returns all of its matches from one `search()`
call, rather than using a callback as the C version's `callback_t` did. This
gives a simpler trait boundary and makes each algorithm directly unit
testable (`assert_eq!(Bf.search("needle", fixture), expected)`).

The registry (in `search/mod.rs`):

```rust
pub fn find_algorithm(code: &str) -> Option<Box<dyn SearchAlgorithm>>;
pub fn list_algorithms() -> &'static [(&'static str, &'static str)]; // (code, name)
```

Per-file thread result, returned via `JoinHandle<PerFileResult>` instead of
the C version's pointer-into-shared-array pattern:

```rust
struct PerFileResult {
    filename: String,
    matches: Vec<Match>,
    bytes: u64,
    elapsed: Option<Duration>,   // None when -t is not set
    error: Option<String>,       // Some(...) if open/read/mmap failed
}
```

A file that fails to open/read/mmap still produces a `PerFileResult` (with
`error: Some(...)`, empty `matches`, `elapsed: None`) rather than aborting
the whole run or panicking the thread — matching the C version's behavior
where one bad file doesn't stop the others.

### File loading (`loader.rs`)

```rust
pub enum Loaded {
    Owned(Vec<u8>),
    Mapped(memmap2::Mmap),
}

impl Loaded {
    pub fn as_bytes(&self) -> &[u8] { ... }
}

pub fn load(path: &Path) -> std::io::Result<Loaded>;
```

`load()` checks the file's size via `metadata()`: under 1GB, opens and
`read()`s the full contents into a `Vec<u8>`; at or above 1GB, opens and
`mmap`s it via `memmap2`. Non-regular files (pipes, `/dev/stdin`) always use
the `read()` path (matching the C version's existing pipe/tty handling),
regardless of size, since they can't be mmap'd.

### Main flow

1. Parse args via `clap`. If `-l`, call `list_algorithms()`, print, exit 0 —
   before requiring `STRING` or files (same ordering as the C version).
2. Resolve `Box<dyn SearchAlgorithm>` via `find_algorithm(&args.algorithm)`;
   unknown code prints an error and exits 1.
3. Resolve the file list: `-f` reads paths from a file via `read_filelist`
   (error and exit 1 if also given positional file arguments); otherwise use
   positional args, or default to `/dev/stdin`. Run the result through
   `expand_paths` to recursively expand any directories into their regular
   files, skipping dotfiles/dotdirs at every depth.
4. Spawn one `std::thread::spawn` per resolved file. Each thread: `loader::load()`
   the file, run `algorithm.search(word, buf)` (wrapped in
   `Instant::now()`/`elapsed()` when `-t` is set, timing only the `search()`
   call — not the load), and return a `PerFileResult`.
5. Join all handles in order, printing each file's matches
   (`filename:lineno line`) as its handle is joined.
6. If `-t`: print `#COMMAND` (reconstructed from `std::env::args()`), then
   `#TIMING_SUMMARY algorithm=<code> files=<n> errors=<n> bytes=<total>
   min=<usec> avg=<usec> max=<usec>` — computed only over results where
   `error.is_none()`, with `errors=` counting the rest. If every file errored
   (or the file set was empty), print an all-zero summary line instead of
   computing min/avg/max — this directly carries forward the div-by-zero fix
   made late in the C version's development.

## Error handling

A `thiserror`-based enum for fatal, pre-thread-spawn errors:

```rust
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("unknown algorithm '{0}'. Run 'greep -l' to see available algorithms.")]
    UnknownAlgorithm(String),
    #[error("cannot combine -f/--filelist with positional file arguments")]
    ConflictingFileArgs,
    #[error("unable to open filelist '{0}': {1}")]
    FilelistOpen(PathBuf, std::io::Error),
}
```

These are printed to stderr and cause `main` to exit with a non-zero status
before any threads are spawned. Per-file errors (a single file failing to
open/read/mmap) are not propagated this way — they're captured into that
file's `PerFileResult.error` and reported as part of the timing summary's
`errors=` count, never aborting the rest of the run.

## CLI compatibility

Same flags as the C version (`-v/--verbose`, `-t/--timing`, `-a/--algorithm
CODE`, `-l/--list`, `-f/--filelist PATH`, positional `STRING [FILES...]`),
parsed with `clap`'s derive API. Help/usage text and error-message wording
follow clap's standard conventions rather than the C version's hand-rolled
usage string — only the flags themselves, the match-line format, and the
`#TIMING`/`#COMMAND`/`#TIMING_SUMMARY` output formats need to match exactly.

## Testing

- Unit tests in each algorithm module (`#[cfg(test)] mod tests`), covering:
  overlapping matches, adjacent matches on the same line, matches on the
  last line with no trailing newline, an absent search word, and a search
  word longer than the buffer. These specifically cover the class of
  multi-match-per-line line-tracking bug found by hand in the C version's
  `find_bf`.
- A shared parameterized test (in `search/mod.rs`'s test module) that runs
  every algorithm returned by the registry against the same fixture set and
  asserts identical results across all of them — automating the "compare
  against a reference algorithm" verification that was done by hand for the
  C version, so the next algorithm added gets this check for free.
- A handful of integration tests via `assert_cmd` driving the compiled
  binary: `-l` output contains expected codes; `-a bogus` exits non-zero with
  an error; `-f` combined with positional args exits non-zero; directory
  arguments expand correctly and skip dotfiles/dotdirs; `-t` output on an
  empty/all-failed file set doesn't crash and shows the all-zero summary.

## Out of scope (for this design)

- stdin "watch" streaming mode (line-buffered incremental scanning for
  unbounded input) — separate design, since it changes the per-file
  processing model rather than just the language.
- Quick Search (or any other) algorithm beyond `bf`/`bmh` — separate design,
  independent of language.
- Configurable mmap threshold (hardcoded at 1GB for this design; could become
  a flag later if needed).
- Keeping the C implementation available for side-by-side comparison — the C
  code is removed from this branch; it remains available via git history,
  `main`, and the other worktree.

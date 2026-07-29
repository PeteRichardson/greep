---
git_sha: 3815ede
generated_at: 2026-07-28
scope: whole repo (Rust rewrite)
decisions_recorded: 2026-07-28
---

# Project Review — greep

Audited at `3815ede`, ~766 LOC of Rust across 8 files. Every finding below was
either read out of the source or reproduced against the release binary; probes
that failed to confirm a suspicion were dropped rather than softened into
hedged findings.

## Executive summary

1. **`bmh` and `bf` return different results for the same input.** A search word
   containing a newline matches under `bmh` and never matches under `bf`
   (reproduced). The cross-algorithm parity test that exists specifically to
   catch this class of bug has no fixture with a newline in the word, so it
   passes.
2. **Exit status is always 0.** No match, every file missing, partial failure —
   all exit 0 (reproduced). This breaks `if greep foo file; then`, `&&` chains,
   and any CI use. Confirmed an omission rather than a design choice; grep's
   0/1/2 convention is the agreed target.
3. **Binary files dump raw bytes to stdout** (reproduced against `/bin/ls`;
   mangles the terminal). No binary detection, no `Binary file X matches`.
4. **No CI exists.** No `.github/` at all. The test suite is good and nothing
   runs it; `cargo fmt --check` and clippy already fail/warn on committed code.
5. **`memmap2 0.9.10` carries RUSTSEC-2026-0186** (unsound, unchecked pointer
   offset). `0.9.11` is available.
6. Output goes through `println!` per match — stdout lock plus flush per line,
   on the hot path of a tool whose entire premise is speed.
7. All matches for all files are retained in memory until the last thread
   joins; a common word over a large tree holds every matching line at once.
8. Unbounded thread spawn (one per file, no pool). Survived 12 000 files in
   testing only because short-lived threads retire faster than they're created
   — the failure mode needs many simultaneously-slow files, and it's a panic.
9. A hand-rolled usage string in `main.rs` duplicates clap's generated help,
   contradicting the design doc's own "help text follows clap conventions".
10. Documentation split is inverted: `README.md` is two lines and documents none
    of the six flags; `CLAUDE.md` carries the real user-facing reference.

No new categories were introduced; all findings fit the existing vocabulary.

## Architectural mental model

greep is a single binary crate that fans out one OS thread per input file and
joins them in spawn order. `options.rs` owns the clap surface and produces a
`ResolvedArgs` after validating the algorithm code, rejecting `-f` combined
with positional files, and defaulting to `/dev/stdin`; `filelist.rs` then
expands that list, recursively walking any directory argument and dropping
dotfiles at every depth. Each worker calls `loader::load`, which reads files
under 1 GB into a `Vec<u8>` and mmaps only at or above that threshold, then
runs a `Box<dyn SearchAlgorithm>` over the resulting byte slice. Algorithms
return `Vec<Match>` (owned line strings) rather than invoking a callback, which
is what makes them directly unit-testable.

The important structural choice is that **nothing prints from inside a worker**.
Results are collected into `PerFileResult` structs, joined in order, and printed
by `main` afterward — so multi-file output is deterministic (verified across
repeated runs), unlike the C predecessor which interleaved. The cost of that
choice is memory: every match is held until every thread finishes. My model
matches the design doc in `docs/specs/2026-06-21-rust-rewrite-design.md`; that
doc is unusually accurate and predicted most of what the code actually does,
which is rare enough to say out loud. The one place code and doc diverge is CLI
help text (F14).

## Findings

| ID | Category | File:Line | Severity | Effort | Description | Recommendation |
|----|----------|-----------|----------|--------|-------------|----------------|
| F1 | Correctness & memory safety | src/search/horspool.rs:24 | High | S | `bmh` matches a word containing `\n` across a line boundary and reports a multi-line "line"; `bf` scans within line bounds and never matches. Reproduced: `greep -a bmh "$(printf 'alpha\nbeta')" f` prints 2 lines, `-a bf` prints nothing. Same input, two answers. | **Decided: line-scoped.** `bf` has the correct semantics; fix `bmh` to reject any candidate whose span crosses a `\n`, keeping `-a` a choice between interchangeable implementations. Add the F2 fixture first. |
| F2 | Test debt | src/search/mod.rs:38 | High | S | The `all_algorithms_agree_on_fixtures` parity test is the designed safety net for exactly F1's bug class (the design doc says so explicitly), but no fixture contains a newline inside the search word, so F1 passes clean. | Add `("alpha\nbeta", b"alpha\nbeta\n", ...)` to `fixtures()`. This single fixture is what makes the harness earn its keep. |
| F3 | UX & CLI ergonomics | src/main.rs:88 | High | S | Exit status is 0 when nothing matched (reproduced). `grep` returns 1. Breaks `if greep ...`, `&&`/`\|\|` chains, and CI gating. | **Decided: adopt grep's 0/1/2.** Track whether any match was emitted; `std::process::exit(1)` when none. Confirmed an omission, not a design choice. |
| F4 | Error handling & observability | src/main.rs:75 | High | S | Per-file failures print `# ERROR: ...` to stderr but do not affect exit status — searching a nonexistent file exits 0 (reproduced). A script cannot tell "no matches" from "the file was missing". | **Decided: exit 2** when any `PerFileResult.error` is set, per grep convention. |
| F5 | UX & CLI ergonomics | src/search/mod.rs:10 | High | M | Binary files emit raw bytes to stdout (reproduced against `/bin/ls`, corrupts terminal state). `Match.line` is a `String` built via `from_utf8_lossy`, so control bytes pass straight through. | Detect NUL in the first block; print `Binary file X matches` once and skip, as grep does. |
| F6 | Data integrity & robustness | src/search/brute_force.rs:8 | Medium | S | An empty search word silently returns zero matches in both algorithms (reproduced, exit 0). `grep ""` matches every line. Silent, not an error. | **Decided: reject at arg-parse time.** Add an `AppError::EmptySearchWord` variant and check in `options::resolve` (src/options.rs:58), so the guard lives at the boundary and neither algorithm needs an empty-word path. |
| F7 | Dependency & config debt | Cargo.toml:12 | Medium | S | `memmap2 0.9.10` is subject to RUSTSEC-2026-0186 (unsound: unchecked pointer offset), confirmed by `cargo audit`. `0.9.11` is published. | Bump to `0.9.11`. Exposure is limited — the mmap path only runs at ≥1 GB — but it's a one-line fix. |
| F8 | Performance & resource hygiene | src/main.rs:73 | Medium | S | `println!` per match acquires the stdout lock and line-flushes for every match, on the hot path of a tool whose stated purpose is speed. | Hold one `BufWriter::new(stdout().lock())` across the whole print loop and `writeln!` into it. |
| F9 | Performance & resource hygiene | src/main.rs:66 | Medium | M | Every match from every file is retained until all threads join, each `Match` owning a heap `String`. Searching a large tree for a common word holds all matching lines simultaneously. | Stream per-file output as each handle joins (join order already gives determinism), so only one file's matches are live at a time. |
| F10 | Architectural decay | src/main.rs:52 | Medium | M | One `std::thread::spawn` per file with no pool or cap; `spawn` panics if the OS refuses. 12 000 files survived testing only because threads retire faster than they are created — the failure mode requires many concurrently-slow (large) files, which is exactly the mmap case. | Cap concurrency at roughly `available_parallelism()` with a simple work queue. Not urgent; do it before adding large-file workloads. |
| F11 | Error handling & observability | src/main.rs:68 | Medium | S | `handle.join().expect("worker thread panicked")` converts one worker panic into a process-wide panic, discarding every other file's already-completed results. | Match on the `Err` and record it as that file's `error`, preserving the rest — the design doc's own "one bad file doesn't stop the others" principle. |
| F12 | Error handling & observability | src/main.rs:91 | Medium | S | `find_algorithm(...).expect("algorithm validated before spawn")` re-resolves the registry inside every worker and panics on a should-be-impossible state, plus one `Box` allocation per file. | Resolve once in `main` and pass the algorithm in, making the invariant structural instead of asserted. |
| F13 | UX & CLI ergonomics | src/options.rs:9 | Medium | S | No `--version` flag; `greep --version` is a clap parse error (exit 2, reproduced). Unusual for any installed CLI. | Add `version` to the `#[command(...)]` attribute — one word. |
| F14 | Documentation drift / UX & CLI ergonomics | src/main.rs:30 | Medium | S | A hand-rolled usage string duplicates clap's generated help and will drift from it. The design doc explicitly states help text should "follow clap's standard conventions rather than the C version's hand-rolled usage string" — code contradicts design. | Delete it; make `search_word` `required_unless_present = "list"` so clap emits usage itself. |
| F15 | IDIOM | src/options.rs:27 | Medium | S | `search_word: Option<String>` plus a manual `let ... else` in `main` re-implements a constraint clap expresses declaratively. True maintenance severity is Low; flagged at the IDIOM floor for language-fluency value. | `#[arg(value_name = "STRING", required_unless_present = "list")]`, then take it as `String`. Removes F14's usage string as a side effect. |
| F16 | IDIOM | src/filelist.rs:31 | Medium | S | `match` used to destructure a single pattern where `let ... else` is idiomatic; clippy-confirmed (`single_match_else`). True maintenance severity Low. | `let Ok(entries) = fs::read_dir(dir) else { ...; return; };` — clippy has an autofix. |
| F17 | IDIOM | src/search/horspool.rs:34 | Medium | S | Bindings `window` and `word` are confusingly similar in the innermost matching loop; clippy-confirmed (`similar_names`). In the one function where an off-by-one is hardest to spot by eye. True maintenance severity Low. | Rename `window` to `win_start` or `offset`. |
| F18 | Type & contract debt | src/loader.rs:30 | Low | S | `metadata.len() as usize` truncates on 32-bit targets (clippy-confirmed) and trusts `metadata` for the `with_capacity` hint — a lying or special file over-allocates. | `usize::try_from(...).unwrap_or(0)`, and cap the pre-allocation hint. |
| F19 | Documentation drift | src/filelist.rs:45 | Low | S | The directory walk's `_ => {}` arm silently drops symlinks — a symlinked file inside a searched directory is never searched (reproduced). | **Decided: skipping is correct** (matches `grep -r`); only the silence is the defect. Downgraded from Medium to Low and recategorised: this is now purely a docs item. Note it in the README flag reference and `--help`; no `-R` follow flag wanted. |
| F20 | UX & CLI ergonomics | src/filelist.rs:41 | Low | M | Dotfile/dotdir skipping is unconditional with no opt-out. Intentional per the design doc, but there is no way to search hidden files at all. | Add `--hidden` when convenient. |
| F21 | Data integrity & robustness | src/filelist.rs:5 | Low | S | `read_filelist` does not dedupe, support comments, or expand `~`. Duplicate lines produce duplicate threads and duplicate output (confirmed: 12 000 identical entries → 12 000 threads over one file). | Dedupe while preserving order; skip `#` comments. |
| F22 | Test debt | tests/cli.rs:1 | Medium | S | No test covers the `/dev/stdin` default path or multi-file output ordering. Both work (verified by hand) and both are load-bearing — ordering is the main behavioral improvement over the C version, and nothing guards it. | Add two `assert_cmd` tests: piped stdin, and a fixed multi-file ordering assertion. |
| F23 | Test debt | src/loader.rs:25 | Medium | M | The mmap branch has no test coverage at all. `MMAP_THRESHOLD_BYTES` is a `const`, so testing it requires a real 1 GB file. Half of the module's reason to exist is unexercised. | Make the threshold a parameter of an inner `load_with_threshold(path, threshold)`; test the mmap branch with a small file and a tiny threshold. |
| F24 | Test debt | tests/cli.rs:61 | Low | S | `-v/--verbose` output and the per-file `#TIMING` lines are untested; only the all-failed timing summary is covered. | Add assertions for `#TIMING` per-file output on a successful run. |
| F25 | IDIOM | src/filelist.rs:59 | Medium | S | Tests build temp paths from `process::id()` and clean up with explicit `remove_*` calls that are skipped whenever an assertion fails, leaking fixtures into the temp dir. True maintenance severity Low. | Use the `tempfile` crate as a dev-dependency; cleanup becomes RAII and survives failures. |
| F26 | Dependency & config debt | Cargo.toml:13 | Low | S | `thiserror = "1"` while 2.0.19 is current. Only used for one three-variant enum, so migration is trivial. | Bump to `2`. |
| F27 | Dependency & config debt | Cargo.toml:1 | Medium | S | No CI whatsoever — no `.github/`. The suite is genuinely good (21 tests) and nothing enforces it; `cargo fmt --check` and clippy already fail/warn on committed code, which is how F16–F18 survived. | Add a workflow running `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`. Highest ratio of value to effort in this table. |
| F28 | Consistency rot | src/filelist.rs:56 | Low | S | `cargo fmt --check` reports drift in committed code (`filelist.rs:56`, `:70`, and others). | Run `cargo fmt`; enforce via F27. |
| F29 | Documentation drift | README.md:1 | Medium | S | The README is two lines and documents none of the six flags, no usage, no build/install. The real user-facing reference lives in `CLAUDE.md` — documentation aimed at an AI agent is better than the documentation aimed at humans. | **Decided: README is primary.** Move the flag reference into `README.md` and reduce `CLAUDE.md` to a pointer, so there is one source of truth. Fold F19's symlink note in while editing. |
| F30 | Architectural decay | src/main.rs:16 | Low | S | `PerFileResult.bytes` is computed unconditionally but consumed only in the timing summary. | Trivial; fold into a timing-only struct or leave and ignore. |
| F31 | Consistency rot | src/main.rs:38 | Low | S | Two error dialects in one binary: hand-written `# ERROR: ...` (a C-era carryover) and clap's standard `error: ...` (reproduced). | **Decided: carryover, unify on clap style.** Confirmed *not* part of the `#TIMING`/`#COMMAND` machine-readable contract, so it can go: switch `main.rs:38`, `main.rs:76` and `filelist.rs:34` to clap's `error:` wording. |
| F32 | Performance & resource hygiene | src/filelist.rs:19 | Low | M | `expand_paths` stats every path and walks every directory serially on the main thread before a single worker starts — a serial prelude to the parallel phase, proportional to tree size. | Only worth addressing if directory search over large trees becomes a real workload. |
| F33 | IDIOM | src/search/brute_force.rs:15 | Medium | S | `pos` is always equal to `line_start` after every iteration — two variables tracking one value in the loop where correctness is subtlest. True maintenance severity Low. | Drop `pos`; or express the whole scan as `buf.split(\|&b\| b == b'\n').enumerate()`, which removes the manual index arithmetic entirely. |
| F34 | Data integrity & robustness | src/filelist.rs:47 | Low | S | `to_string_lossy().into_owned()` mangles non-UTF-8 filenames into U+FFFD, producing a path that then fails to open. Near-unreachable on macOS (APFS enforces UTF-8); a real bug on Linux. | Carry `PathBuf` through instead of `String` if portability is ever a goal. |

34 findings. I stopped where the real ones stopped — for 766 LOC, padding toward
the usual 30–80 band would have meant inventing filler.

## Top 5 — if you fix nothing else

### 1. Exit codes (F3, F4)

The single highest-impact fix. Right now greep cannot be used in a shell
conditional at all.

```rust
// main.rs, end of main()
let any_match = results.iter().any(|r| !r.matches.is_empty());
let any_error = results.iter().any(|r| r.error.is_some());
std::process::exit(if any_error { 2 } else if any_match { 0 } else { 1 });
```

Add a CLI test per branch — this is exactly the kind of thing that silently
regresses.

### 2. Algorithm divergence (F1 + F2)

Two algorithms disagreeing is worse than either being wrong, because `-a` is
advertised as an interchangeable choice. Fix the fixture first — it fails, then
you know the fix works:

```rust
// search/mod.rs fixtures()
("alpha\nbeta", b"alpha\nbeta\ngamma\n" as &[u8], vec![/* agreed expectation */]),
```

Then make BMH reject a candidate whose span crosses a `\n` — line-scoped
semantics are the confirmed intent, so `bf` is the reference and `bmh` is the
one that moves.

### 3. Binary detection (F5)

```rust
// before searching, in run_file
if loaded.as_bytes().iter().take(8192).any(|&b| b == 0) {
    // report "Binary file {filename} matches" if any match, then skip printing lines
}
```

Cheap, and it stops the tool from corrupting the user's terminal.

### 4. Buffered output + streaming (F8, F9)

One `BufWriter` around a held `StdoutLock` for the whole print loop fixes F8.
Printing each file's matches as its handle joins fixes F9 without losing
determinism, since join order is already spawn order.

### 5. CI (F27)

```yaml
# .github/workflows/ci.yml
- run: cargo test
- run: cargo clippy --all-targets -- -D warnings
- run: cargo fmt --check
```

Every one of F16, F17, F18, F28 would have been caught before commit.

## Quick wins

Low effort, Medium-or-higher severity:

- [ ] F7 — bump `memmap2` to 0.9.11 (RUSTSEC advisory)
- [ ] F27 — add the three-line CI workflow
- [ ] F2 — add the newline fixture to the parity test
- [ ] F3 / F4 — exit codes
- [ ] F13 — add `version` to the clap command attribute
- [ ] F14 / F15 — `required_unless_present = "list"`, delete the usage string
- [ ] F16 — `cargo clippy --fix` (autofix available)
- [ ] F28 — `cargo fmt`
- [ ] F6 — reject an empty search word in `options::resolve`
- [ ] F29 — move the flag reference into the README (fold in F19's symlink note)
- [ ] F31 — unify hand-written errors on clap's `error:` style

## Things that look bad but are actually fine

- **`unsafe { Mmap::map(&file) }` (loader.rs:26).** The obvious thing to flag in
  a Rust audit, and wrong to flag. `memmap2`'s API is unsafe because another
  process can truncate the file and hand you a SIGBUS — that's inherent to mmap,
  not a defect here. The only actionable part is the version bump (F7).
- **The 1 GB mmap threshold making mmap look like dead code.** It does mean mmap
  effectively never runs in normal use, which reads as a red flag. The design doc
  justifies it concretely: `mmap` defers page-ins into the timed `search()` call
  and so pollutes `-t` measurements, while remaining the right choice at sizes
  where the kernel needs to reclaim clean pages. That's a real argument, and the
  threshold is listed as a known future flag. Intentional.
- **Collecting all results before printing (main.rs:66).** Looks like the classic
  "should have streamed" mistake, and F9 does flag its memory cost — but the
  ordering it buys is a genuine improvement over the C version's interleaved
  output, verified deterministic across repeated runs. Fix the memory by
  streaming *in join order*; do not fix it by printing from workers.
- **`Box<dyn SearchAlgorithm>` dynamic dispatch.** Reflex says virtual call in a
  search tool. It resolves once per *file*, not per byte or per line — the inner
  loops are static. Irrelevant to performance.
- **`search_word.clone()` per spawned thread (main.rs:61).** One small `String`
  clone per file, against a backdrop of opening and reading that file. `Arc`
  would be strictly more code for unmeasurable gain.
- **One thread per file with no pool (F10).** Flagged, but deliberately at
  Medium rather than High: I tried to break it at 12 000 files and could not,
  because threads retire faster than they spawn. Don't rewrite this into a
  thread pool as a panic response; do it when large-file workloads arrive.
- **`greep` shadowing `grep`'s name and flags.** Deliberate — it's the project's
  entire premise.

## Maintainer decisions (resolved 2026-07-28)

Every question this audit raised has been answered. Recorded here because the
answers are design intent that isn't derivable from the code — a future reader
(or a regenerated version of this report) would otherwise have to re-ask them.

| # | Question | Decision | Effect on findings |
|---|----------|----------|--------------------|
| 1 | Are exit codes an intentional omission? | **Adopt grep's 0/1/2** — 0 matched, 1 no match, 2 any file errored. | F3, F4 confirmed High; both are now specified, not speculative. |
| 2 | Is `# ERROR:` a deliberate machine-readable prefix? | **No — C-era carryover.** Not part of the `#TIMING`/`#COMMAND`/`#TIMING_SUMMARY` contract. | F31 stands; unify on clap's `error:` style. Those three `#`-prefixed formats remain a real output contract and must not be touched. |
| 3 | Is `bmh` line-scoped or a raw byte searcher? | **Line-scoped.** `-a` selects between interchangeable implementations. | F1 fix lands in `bmh` (`bf` is the reference). F2's fixture asserts agreement, not divergence. |
| 4 | Symlinks in directory walks? | **Skip, matching `grep -r`** — no follow flag wanted. | F19 downgraded Medium → Low and recategorised as Documentation drift. Behavior is correct; only the silence was the defect. |
| 5 | Is `CLAUDE.md` the primary user documentation? | **No — README is primary;** `CLAUDE.md` becomes a pointer. | F29 stands as written. |
| 6 | Empty search word behavior? | **Reject at arg-parse time** rather than matching every line. | F6's fix moves out of the algorithms and into `options::resolve`; neither algorithm needs an empty-word path. |

No open questions remain. If this report is regenerated against a later commit,
carry this table forward — these are decisions, not observations, and rescanning
the code will not recover them.

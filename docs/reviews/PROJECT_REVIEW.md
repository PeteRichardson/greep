---
git_sha: 43ea602
generated_at: 2026-10-02
scope: whole repo (Rust rewrite)
decisions_recorded: 2026-07-28
previous_audit: 3815ede (2026-07-28, F1–F34)
---

# Project Review — greep (repeat run)

Audited at `43ea602`, ~1 150 LOC of Rust across 5 source files plus 522 lines of
integration tests. Repeat run of the 2026-07-28 audit (at `3815ede`): every one
of the 34 findings was re-verified against the current tree. **30 are RESOLVED,
4 remain open** (F10, F12, F20, F32 — all already filed as GitHub issues
#15, #17, #25, #37). This run adds **3 new findings (F35–F37)**, two of them
reproduced against the release binary.

## Executive summary

1. **`-t` panics (exit 101) when any argv element is non-UTF-8** (F36, new, High).
   `print_timing_summary` rebuilds `#COMMAND` via `std::env::args()`, which
   unwraps on invalid UTF-8. Reproduced: `greep -t word caf\xff.txt` prints a
   clean per-file error, then panics in `env::args`. This re-breaks, for the
   `-t` path only, exactly what `3084fd0`/issue #39 just fixed for the search
   path. One-line-class fix: `args_os()`.
2. **An unreadable directory in the walk prints `error:` but exits 1, not 2**
   (F35, new, High). `walk_directory`'s `read_dir` failure is printed on the
   main thread *before* workers exist, so it never reaches `any_error`.
   Reproduced: `chmod 000 dir; greep word dir` → exit 1. The README's exit
   status table promises `2` for an unreadable input; scripts can't tell
   "permission denied" from "no matches".
3. **The prior audit was fully worked.** All of F1–F9 (algorithm divergence,
   exit codes, binary bytes, empty word, CVE dep, unbuffered stdout, match
   retention) and F11–F34 are resolved and test-guarded. The `3815ede` tree was
   the worst this codebase has been; nothing from that list regressed.
4. **Still open since the last audit** (all Medium/Low, all already issues):
   F10 unbounded thread spawn (#15), F12 per-worker `find_algorithm(...).expect`
   re-resolution (#17), F20 no `--hidden` opt-out (#25), F32 serial directory
   pre-walk (#37).
5. **Hygiene is now genuinely good.** `cargo fmt --check` clean, `cargo clippy
   --all-targets` zero warnings (pedantic is in the `[lints]` table), 70 tests
   pass, `cargo audit` clean, `cargo machete` finds no unused deps, CI enforces
   fmt + clippy + test with an advisory pedantic job.
6. **The tactical pipeline has 4 open issues** from the `/code-review` PR-mode
   passes (#53–#56), including manifest double-spelling duplicate output (#54),
   which I independently reproduced (`sub` + `./sub` in one manifest prints the
   same line twice).
7. **`docs/.gitignore` is untracked** (F37, Low): the rule that keeps
   `/code-review` snapshots out of git exists only on this machine; a fresh
   clone will happily commit them on a bulk `git add docs/`.

No new categories were introduced; all findings fit the established vocabulary.

## Architectural mental model

Structurally unchanged since the 2026-07-28 audit, and still matching
`docs/specs/2026-06-21-rust-rewrite-design.md`. One clap surface
(`options.rs`) resolves to `ResolvedArgs`; `filelist.rs` reads the `-f` manifest
(now with comment/tilde/dedup rules) and expands directories; workers
`loader::load` (read vs mmap at 1 GiB) and run a `Box<dyn SearchAlgorithm>`;
`main.rs` joins in spawn order, streams each file's matches through one
`BufWriter`, and folds everything into `RunTotals`.

The changes since the audit were **behavioral, not structural**: the exit
status became grep's 0/1/2 with error-outranks-match; output streams per join
(`RunTotals` replaces the retained `PerFileResult` vector); timing became
opt-in (`TimingInfo: Option`, no measurement without `-t`); paths are `PathBuf`
end to end; binary files are sniffed and reported, not dumped. Two new
concentrations of logic arrived: manifest semantics in `filelist.rs` (the
comments/tilde/dedup rules are subtle and well-tested) and the timing summary
in `main.rs`. The two new findings live exactly at the seams those changes
created — argv reconstruction in the timing path, and main-thread walk errors
that bypass the per-file error channel.

## Findings

Status: `RESOLVED` = fixed since `3815ede` and re-verified; `OPEN` = carried,
still present; `NEW` = first seen this run.

| ID | Status | Category | File:Line | Severity | Effort | Description | Recommendation |
|----|--------|----------|-----------|----------|--------|-------------|----------------|
| F1 | RESOLVED | Correctness & memory safety | src/search/horspool.rs:11 | High | S | `bmh` matched a `\n`-containing word across line boundaries, disagreeing with `bf`. | Fixed: `word.contains(&b'\n')` rejected up front; newline fixture guards it in `search/mod.rs` tests. |
| F2 | RESOLVED | Test debt | src/search/mod.rs:63 | High | S | Parity test had no newline-in-word fixture. | Fixed: `("alpha\nbeta", b"alpha\nbeta\n", vec![])` in `fixtures()`. |
| F3 | RESOLVED | UX & CLI ergonomics | src/main.rs:15 | High | S | Exit status always 0. | Fixed: `EXIT_MATCH`/`EXIT_NO_MATCH`/`EXIT_ERROR`, error outranks match; CLI-tested. |
| F4 | RESOLVED | Error handling & observability | src/main.rs:76 | High | S | Per-file failures didn't affect exit status. | Fixed: `any_error` → exit 2; tested. |
| F5 | RESOLVED | UX & CLI ergonomics | src/loader.rs:15 | High | M | Binary files dumped raw bytes. | Fixed: `looks_binary` (8 KiB sniff) + `Binary file X matches`; verified against a NUL file. |
| F6 | RESOLVED | Data integrity & robustness | src/options.rs:57 | Medium | S | Empty search word silently matched nothing. | Fixed: clap requires the word, `AppError::EmptySearchWord` at the boundary; tested. |
| F7 | RESOLVED | Dependency & config debt | Cargo.toml | Medium | S | `memmap2 0.9.10` RUSTSEC-2026-0186. | Fixed: 0.9.x line, `cargo audit` clean this run. |
| F8 | RESOLVED | Performance & resource hygiene | src/main.rs:104 | Medium | S | `println!` lock+flush per match. | Fixed: one `BufWriter` over a held `StdoutLock`, final explicit flush. |
| F9 | RESOLVED | Performance & resource hygiene | src/main.rs:36 | Medium | M | All matches retained until last join. | Fixed: matches printed and dropped per join; only `RunTotals` accumulates. |
| F10 | OPEN | Architectural decay | src/main.rs:92 | Medium | M | One `std::thread::spawn` per file, no pool or cap; `spawn` panics if the OS refuses. Still filed as issue #15. Risk case unchanged: many concurrently-slow (≥1 GiB mmap) files. | Cap at `available_parallelism()` with a work queue before large-file workloads arrive. Don't do it as a panic response. |
| F11 | RESOLVED | Error handling & observability | src/main.rs:120 | Medium | S | Worker panic took down the process. | Fixed: `join()` `Err` becomes that file's `error` via `panic_message()`. |
| F12 | OPEN | Error handling & observability | src/main.rs:204 | Medium | S | `find_algorithm(...).expect("algorithm validated before spawn")` re-resolves the registry per worker; `algorithm_code` is cloned per file for it. Issue #17. | Resolve once in `run()` and hand the `Box<dyn SearchAlgorithm>` (or a `&'static` factory result) to workers, making the invariant structural. |
| F13 | RESOLVED | UX & CLI ergonomics | src/options.rs:19 | Medium | S | No `--version`. | Fixed: `version` on `#[command]`; CLI tests assert `-V`/`--version`. |
| F14 | RESOLVED | Documentation drift / UX & CLI ergonomics | src/options.rs:17 | Medium | S | Hand-rolled usage string duplicated clap's help. | Fixed: deleted; a CLI test asserts clap's `Usage:` and the absence of the old string. |
| F15 | RESOLVED | IDIOM | src/options.rs:38 | Medium | S | Manual `let-else` in `main` re-implemented a clap constraint. | Fixed: `required_unless_present = "list"`; `resolve` folds `None` with a documented `unwrap_or_default` (the `Option` is now what clap itself requires). |
| F16 | RESOLVED | IDIOM | src/filelist.rs:106 | Medium | S | `match` for a single pattern in `walk_directory`. | Fixed: `let ... else`. |
| F17 | RESOLVED | IDIOM | src/search/horspool.rs:24 | Medium | S | Confusingly similar `window`/`word` bindings in BMH's innermost loop. | Fixed: `win_start`; clippy pedantic is now warning-free. |
| F18 | RESOLVED | Type & contract debt | src/loader.rs:52 | Low | S | `metadata.len() as usize` truncation + lying-hint over-allocation. | Fixed: `usize::try_from(len.min(threshold)).unwrap_or(0)`, commented. |
| F19 | RESOLVED | Documentation drift | src/options.rs:10 | Low | S | Symlink skipping was silent. | Fixed: `DIRECTORY_WALK_HELP` in `--help` + README known limitations. |
| F20 | OPEN | UX & CLI ergonomics | src/filelist.rs:112 | Low | M | Dotfile skipping unconditional, no `--hidden` opt-out. Issue #25. Intentional per design doc; flag when convenient. | Add `--hidden` if hidden-file search ever becomes a real request. |
| F21 | RESOLVED | Data integrity & robustness | src/filelist.rs:66 | Low | S | Manifest: no dedupe, no comments, no `~`. | Fixed: all three, with 8 unit tests. Follow-on sharp edges tracked tactically as #54/#55/#56. |
| F22 | RESOLVED | Test debt | tests/cli.rs | Medium | S | No stdin-default or multi-file ordering tests. | Fixed: piped-stdin cases (incl. 60 KB) + ordering test. |
| F23 | RESOLVED | Test debt | src/loader.rs:39 | Medium | M | mmap branch untested (1 GiB `const`). | Fixed: `load_with_threshold` injection; branch tested at 12 bytes, `>=` pinned by a 12/13 pair. |
| F24 | RESOLVED | Test debt | tests/cli.rs:435 | Low | S | `-v` and per-file `#TIMING` untested. | Fixed: incl. `timing_and_verbose_are_independent`. |
| F25 | RESOLVED | IDIOM | src/filelist.rs:126 | Medium | S | `process::id()` fixture paths with cleanup skipped on failure. | Fixed: `tempfile::TempDir` everywhere; no `process::id()` remains. |
| F26 | RESOLVED | Dependency & config debt | Cargo.toml | Low | S | `thiserror = "1"`. | Fixed: `2`. |
| F27 | RESOLVED | Dependency & config debt | .github/workflows/ci.yml | Medium | S | No CI. | Fixed: fmt + clippy (blocking) + test + advisory pedantic, with concurrency cancellation. |
| F28 | RESOLVED | Consistency rot | src/filelist.rs | Low | S | `cargo fmt --check` drift in committed code. | Fixed: clean this run, enforced in CI. |
| F29 | RESOLVED | Documentation drift | README.md | Medium | S | Two-line README; real reference in CLAUDE.md. | Fixed: full README reference; CLAUDE.md is now a one-line `@AGENTS.md` pointer. |
| F30 | RESOLVED | Architectural decay | src/main.rs:27 | Low | S | `bytes` measured unconditionally, consumed only under `-t`. | Fixed: `TimingInfo: Option`, measured only when `-t`. |
| F31 | RESOLVED | Consistency rot | src/main.rs:81 | Low | S | Two error dialects (`# ERROR:` vs clap's `error:`). | Fixed: all per-file and resolve errors use `error:`; the `#`-prefix contract is reserved for timing. |
| F32 | OPEN | Performance & resource hygiene | src/filelist.rs:94 | Low | M | `expand_paths` stats and walks the whole tree serially before any worker starts. Issue #37. | Only worth addressing if large-tree directory search becomes a real workload (the walk is I/O-bound and would parallelize trivially). |
| F33 | RESOLVED | IDIOM | src/search/brute_force.rs:10 | Medium | S | `pos` ≡ `line_start` double-tracking in the bf scan. | Fixed: scan expressed as `split(|&b| b == b'\n').enumerate()`. |
| F34 | RESOLVED | Data integrity & robustness | src/filelist.rs | Low | S | `to_string_lossy` mangled non-UTF-8 names into unopenable paths. | Fixed: `PathBuf` end to end, dotfile check on `as_encoded_bytes()`; regression test (real on Linux, early-returns on APFS). |
| F35 | **NEW** | Error handling & observability | src/filelist.rs:106 | High | S | An unreadable directory in the walk prints `error: unable to open directory` on the **main thread, before any worker exists**, so it never reaches `any_error` — the run exits **1** (no match), not **2**. Reproduced: `chmod 000 d; greep word d` → error printed, exit 1. Violates the README Exit Status table (`2` = an error occurred, unreadable file). A script cannot distinguish "permission denied" from "nothing matched". | Give the walk an error channel: `walk_directory`/`expand_paths` return (or accumulate) their failures, and `run()` folds them into `any_error` (exit 2) and the error output. Add a CLI test (mode-revoked dir on the CI runner). |
| F36 | **NEW** | Correctness & memory safety | src/main.rs:275 | High | S | `std::env::args()` panics (`called Result::unwrap()`) if **any** argv element is non-UTF-8, and `print_timing_summary` calls it — so `greep -t word <non-UTF-8 name>` panics with a raw Rust note (exit 101) *after* printing clean results. Reproduced. Directly contradicts `3084fd0`/issue #39, whose entire point is that non-UTF-8 filenames are first-class; the search path handles them, the `#COMMAND` line doesn't. | `std::env::args_os().map(|a| a.to_string_lossy().into_owned())` — `#COMMAND` is a human/machine-readable echo, and lossy display is exactly right for it. Add a test that constructs a non-UTF-8 `PathBuf` argument (unix `From<OsString>`/`as_bytes`) and runs with `-t`. |
| F37 | **NEW** | Dependency & config debt | docs/.gitignore | Low | S | The ignore rule keeping `/code-review` snapshots (`reviews/code-review*.md`) out of git lives in an **untracked** file, so it exists only on this machine. A fresh clone has no such rule, and a bulk `git add docs/` there commits the deliberately-ephemeral snapshots (the comments in the file say they "go stale as soon as the code moves"). The guardrail for a documented convention is itself unguarded. | Commit `docs/.gitignore`. (Or, if the local-only state is deliberate, say so in the file and add the pattern to the root `.gitignore` instead — see Open questions.) |

37 rows: 30 resolved, 4 open, 3 new. For a 1 150-LOC crate that is about
where the real findings stop; I did not pad.

## Related tactical findings

The two `/code-review` reports in `docs/reviews/`
(`code-review_PR46_2026-07-30.md`, `code-review_PR51_2026-07-30.md`) are
**PR-mode** snapshots (frontmatter `pr:`) of branch heads, so they are out of
scope for this audit and are not re-quoted here. The live record from that
pipeline is the four still-open issues:

- **#54** — manifest dedupe runs *before* `expand_paths`, so `sub` + `./sub`
  (or a directory listed under two spellings) reach the worker pool twice and
  print the same match line twice. **Independently reproduced this run.**
- **#55** — comment lines in a manifest are skipped silently; no `-v` note.
- **#56** — the `read_filelist` tilde tests read the real `$HOME` and assert
  paths shaped from it, instead of using the injectable
  `expand_tilde_against` the unit tests already use.
- **#53** — `--version` output is asserted only as `contains(version)`, not the
  README-documented `greep <version>` format.

## Top 5 — if you fix nothing else

### 1. F36 — stop panicking on non-UTF-8 argv under `-t` (S)

A raw std panic in a tool that just spent a PR becoming non-UTF-8-safe.

```rust
// src/main.rs, print_timing_summary
- let command: Vec<String> = std::env::args().collect();
+ // `args_os` + lossy: a non-UTF-8 *filename* reaches argv fine (paths are
+ // PathBuf end to end), and `#COMMAND` is an echo — display-quality is enough.
+ let command: Vec<String> = std::env::args_os()
+     .map(|a| a.to_string_lossy().into_owned())
+     .collect();
```

Test: unix-only, build a `PathBuf` from `OsString` bytes containing `0xFF`,
run with `-t`, assert exit 2 (file missing) rather than 101.

### 2. F35 — route walk failures into the exit status (S)

```rust
// src/filelist.rs — the walk reports, the caller decides
- let Ok(entries) = fs::read_dir(dir) else {
-     eprintln!("error: ..."); return;
- };
+ fn walk_directory(dir: &Path, out: &mut Vec<PathBuf>, errors: &mut Vec<String>) { /* push */ }
// resolve() or run(): collect walk errors, print them, set any_error → exit 2
```

Keep the per-directory "search the rest anyway" behavior; only the exit code
changes. CLI test with a `chmod 000` dir (works on the ubuntu runner; guard
for root).

### 3. F12 — resolve the algorithm once (S)

Open since the last audit. In `run()`: `let algorithm = find_algorithm(&code).expect(...)`
after `resolve()` (which already validated it), and pass `&algorithm` into
`run_file`; drop the per-file `algorithm_code` clone and the in-worker re-lookup.
The `expect` stays but sits on the validated main-thread path where it truly is
unreachable.

### 4. F10 — cap thread concurrency (M)

`let pool = thread_pool(std::thread::available_parallelism()...)` with a channel
of file names; workers pull until the queue drains. Determinism is preserved
trivially because output order comes from *join/emit order*, so collect results
into a `HashMap<PathBuf, PerFileResult>` and emit in argument order. Do this
before, not after, a ≥1 GiB multi-file workload exposes the `spawn`-panic mode.

### 5. F37 — commit `docs/.gitignore` (S)

One `git add docs/.gitignore && git commit`. Without it the
"ephemeral snapshots" convention exists only on one machine.

## Quick wins

Low effort × Medium-or-higher severity:

- [ ] **F36** — `args_os()` in `print_timing_summary` (High, S)
- [ ] **F35** — walk errors → exit 2 (High, S)
- [ ] **F12** — resolve algorithm once, pass into workers (Medium, S)

(Also S, listed for completeness though Low-severity: F37 — commit
`docs/.gitignore`.)

## Things that look bad but are actually fine

- **`unsafe { Mmap::map(&file) }` (src/loader.rs:45).** Carried from the last
  audit and still wrong to flag: the `unsafe` is `memmap2`'s API surface
  (external truncation → SIGBUS), inherent to mmap, not a defect in this code.
- **The 1 GiB mmap threshold making mmap look like dead code.** Design doc
  justifies it (mmap page-ins would pollute the `-t` measurement at small
  sizes; it's the right call where the kernel must reclaim clean pages).
  `load_delegates_with_the_one_gibibyte_threshold` even pins the constant.
  Intentional.
- **`search_word.clone()` and `algorithm_code.clone()` per spawned thread
  (src/main.rs:97-100).** Two small `String` clones against a backdrop of
  opening, stat-ing, and reading that file. `Arc` is strictly more code for
  unmeasurable gain. (F36/F12's fixes touch these lines for other reasons.)
- **`Box<dyn SearchAlgorithm>` dynamic dispatch in a "fast search tool".**
  Resolves once per *file*; the inner loops are monomorphic. Irrelevant.
- **Manifest `trim_end_matches(['\r', '\n'])` (src/filelist.rs:74) mangles a
  legal filename with a trailing CR.** This is CRLF support doing its job; a
  trailing-LF name is unrepresentable in a line-based manifest anyway, and a
  trailing-CR name is rarer than Windows line endings. Deliberate. (Document
  it — see Open questions.)
- **`args.search_word.unwrap_or_default()` in `resolve` (src/options.rs:60)
  looks like it papers over a clap invariant.** The comment says exactly what
  it is: `required_unless_present = "list"` makes `None` unreachable in
  practice, and the fold degrades gracefully instead of asserting.
- **`expanded.clone()` per manifest line for the dedupe set
  (src/filelist.rs:87).** Manifests are cold paths; one `PathBuf` clone per
  line next to a `HashSet` insert is not a finding.
- **`# Processing file {i}` prints at *spawn* time (src/main.rs:90), and an
  unreadable directory's error line can precede the `# Searching for` line.**
  It's a spawn trace, not a completion log, and both are stderr cosmetics.
  (F35's fix will move the walk error; the ordering quirk itself is not
  worth a change.)
- **Clippy pedantic is advisory-only in CI** (`.github/workflows/ci.yml`).
  The workflow comment states the rationale — advisory job, never a required
  check — and local `cargo clippy` applies the same warn-level policy via the
  `[lints]` table. Consistent and documented; today it happens to be
  warning-free anyway.
- **One thread per file, no pool (F10).** Still flagged, still deliberately
  Medium: the last audit stress-tested 12 000 files and the retirement rate
  held. Not a "fix now" — a "fix before large-file workloads" (Top 5, #4).

## Maintainer decisions (resolved 2026-07-28 — carried forward)

Carried unchanged from the previous run per the repeat-run protocol: these are
decisions, not observations, and rescanning the code will not recover them.

| # | Question | Decision | Effect on findings |
|---|----------|----------|--------------------|
| 1 | Are exit codes an intentional omission? | **Adopt grep's 0/1/2** — 0 matched, 1 no match, 2 any file errored. | F3/F4 (resolved). F35 is the same contract applied to a path the original decision didn't cover: a *walk* failure, not a *file* failure. |
| 2 | Is `# ERROR:` a deliberate machine-readable prefix? | **No — C-era carryover.** The `#` prefix belongs to `#TIMING`/`#COMMAND`/`#TIMING_SUMMARY` only. | F31 (resolved). Consequence for F36: `#COMMAND` must stay lossy-display-safe rather than switching to a non-`#` form. |
| 3 | Is `bmh` line-scoped or a raw byte searcher? | **Line-scoped.** `-a` selects interchangeable implementations. | F1/F2 (resolved); the newline fixture enforces it permanently. |
| 4 | Symlinks in directory walks? | **Skip, matching `grep -r`** — no follow flag. | F19 (resolved). Explicit-argument symlinks are still followed; the asymmetry is documented in `--help` and the README. |
| 5 | Is `CLAUDE.md` the primary user documentation? | **No — README is primary.** | F29 (resolved; CLAUDE.md is now a pointer to AGENTS.md). |
| 6 | Empty search word behavior? | **Reject at arg-parse time.** | F6 (resolved). |

## Open questions for the maintainer

1. **Manifest normalization** (relates to open issue #54): should `sub` and
   `./sub` collapse to one entry? String-level dedupe (current, documented)
   misses lexical twins; canonicalizing via `fs::canonicalize` would catch
   them but also rewrites every path (relative → absolute) and breaks the
   "the path you wrote is the path you get" printing. What's the intended
   contract — "identical lines dedupe" or "identical *files* dedupe"?
2. **Trailing-CR manifest rule**: the README's manifest section documents
   leading-whitespace, comments, tilde, and dedupe, but not that a trailing
   CR is trimmed (CRLF support). Document it, or leave it as an
   implementation detail?
3. **`docs/.gitignore`** (F37): is the untracked state a deliberate local
   guardrail, or an uncommitted file? If deliberate, the pattern should move
   to the root `.gitignore` so the convention survives a clone; if accidental,
   commit it.

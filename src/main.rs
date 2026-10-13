mod filelist;
mod loader;
mod options;
mod pool;
mod search;

use std::fmt::Display;
use std::io::{BufWriter, Write};
use std::num::NonZeroUsize;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Parser;

use filelist::WalkItem;
use options::{resolve, Args, ResolvedArgs};
use search::{Match, SearchAlgorithm};

/// Exit codes follow grep: 0 = matched, 1 = no match, 2 = an error occurred.
/// An error outranks a match, so a run that both matched and failed reports 2.
const EXIT_MATCH: i32 = 0;
const EXIT_NO_MATCH: i32 = 1;
const EXIT_ERROR: i32 = 2;

/// Everything the timing summary needs from one file, and nothing else. Absent
/// unless `-t` was given: without it neither field has a consumer, so neither is
/// computed. `bytes` in particular used to be measured on every run and read only
/// when timing was on.
struct TimingInfo {
    bytes: u64,
    elapsed: Duration,
}

struct PerFileResult {
    filename: PathBuf,
    matches: Vec<Match>,
    /// The file contains a NUL in its first block. Its matches are still counted
    /// for exit status, but the matching lines are never written out — they are
    /// arbitrary bytes and would corrupt the terminal.
    binary: bool,
    timing: Option<TimingInfo>,
    error: Option<String>,
}

impl PerFileResult {
    /// A file that produced no search at all: it could not be loaded, or the
    /// search panicked.
    fn failed(filename: PathBuf, error: String) -> Self {
        Self {
            filename,
            matches: Vec::new(),
            binary: false,
            timing: None,
            error: Some(error),
        }
    }
}

/// What one walk item came to by the time it reaches the output.
enum Outcome {
    File(PerFileResult),
    WalkError(String),
}

/// Running totals over every file, accumulated as each one's matches are printed
/// and dropped. Holding this instead of the `PerFileResult`s is what lets the
/// match text be freed per file rather than accumulating until the end.
#[derive(Default)]
struct RunTotals {
    files: usize,
    /// Files that could not be read plus directories the walk could not open,
    /// so the -t summary's `errors=` agrees with the exit status.
    errors: usize,
    /// Files with at least one match — `files` counts files *searched*, which is
    /// not the same question.
    files_matched: usize,
    /// Matching lines across every file, binary ones included: their matches
    /// count even though the lines themselves are never written out.
    matches: u64,
    /// One entry per file that was searched successfully. Empty when `-t` is off.
    timings: Vec<TimingInfo>,
}

fn main() {
    // `run` returns the exit code so that every buffered write is flushed by the
    // time we get here — `std::process::exit` does not run destructors.
    std::process::exit(run());
}

fn run() -> i32 {
    let args = Args::parse();

    if args.list {
        options::print_algorithm_list();
        return EXIT_MATCH;
    }

    let resolved = match resolve(args) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_ERROR;
        }
    };

    if resolved.verbose {
        print_verbose_preamble(&resolved);
    }

    let ResolvedArgs {
        verbose,
        timing,
        algorithm_code,
        algorithm,
        search_word,
        paths,
        ..
    } = resolved;

    // Wall-clock covers the whole run: walking, loading, searching and writing.
    // Taken only when it has a consumer, for the same reason `TimingInfo` is
    // optional. Started here rather than in `main` so it excludes process start
    // and argument parsing, which greep cannot influence.
    let started = timing.then(Instant::now);

    // One lock and one buffer for the whole run, rather than a lock and a flush
    // per match.
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut totals = RunTotals::default();

    let mut file_index = 0;
    let jobs = filelist::walk(paths).inspect(|item| {
        if let (true, WalkItem::File(filename)) = (verbose, item) {
            eprintln!("# Processing file {file_index}: {}", filename.display());
            file_index += 1;
        }
    });

    // The thread count is fixed, not one per file: a thread per file has no
    // upper bound, and `spawn` panics when the OS refuses one more.
    let workers = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);

    // The pool hands results back in walk order, which is argument order, so the
    // output is deterministic. Each file's matches are emitted and dropped as
    // its result lands, so the match text never accumulates across the run.
    pool::run_ordered(
        jobs,
        workers,
        |item| match item {
            WalkItem::File(filename) => {
                Outcome::File(search_file(filename, &search_word, algorithm, timing))
            }
            WalkItem::Error(err) => Outcome::WalkError(err),
        },
        |outcome| {
            let result = match outcome {
                Outcome::File(result) => result,
                Outcome::WalkError(err) => {
                    // Anything the walk could not read is an error just like
                    // a file a worker could not read: reported, counted, and
                    // exit status 2. It is not a file searched.
                    report_error(&mut out, &mut totals, err);
                    return;
                }
            };

            // Bound once: `Path` has no `Display`, and calling `.display()` at
            // each use site is what pushed this formatting onto extra lines.
            let name = result.filename.display();

            if result.binary {
                // Report that it matched without emitting the bytes themselves.
                // A file with no match prints nothing at all, as grep does.
                if !result.matches.is_empty() {
                    let _ = writeln!(out, "Binary file {name} matches");
                }
            } else {
                for m in &result.matches {
                    let _ = writeln!(out, "{name}:{} {}", m.line_number, m.line);
                }
            }
            totals.files += 1;
            if !result.matches.is_empty() {
                totals.files_matched += 1;
                totals.matches += result.matches.len() as u64;
            }

            if let Some(err) = &result.error {
                report_error(&mut out, &mut totals, format_args!("{name}: {err}"));
            }

            if let Some(info) = result.timing {
                let _ = out.flush();
                eprintln!("#TIMING {:8} {name}", info.elapsed.as_micros());
                totals.timings.push(info);
            }
            // `result`, and with it this file's match text, is dropped here.
        },
    );

    if let Err(e) = out.flush() {
        eprintln!("error: writing to stdout: {e}");
        return EXIT_ERROR;
    }

    if let Some(started) = started {
        print_timing_summary(&algorithm_code, &totals, started.elapsed());
    }

    if totals.errors > 0 {
        EXIT_ERROR
    } else if totals.files_matched > 0 {
        EXIT_MATCH
    } else {
        EXIT_NO_MATCH
    }
}

/// Reports one error and counts it. The count is the only error state, so the
/// exit status and the -t summary's `errors=` cannot disagree.
fn report_error(out: &mut impl Write, totals: &mut RunTotals, msg: impl Display) {
    // Keep stdout and stderr readable relative to each other; this is once per
    // error, not once per match.
    let _ = out.flush();
    eprintln!("error: {msg}");
    totals.errors += 1;
}

/// Recover the message from a panicking search.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .map_or_else(
            || "worker thread panicked".to_string(),
            |s| format!("worker thread panicked: {s}"),
        )
}

/// The `-v` lines that come before any file: what is searched for, and each
/// manifest comment skipped (#55), since a skipped `#draft.txt` line is
/// otherwise visible only as a missing result.
fn print_verbose_preamble(resolved: &ResolvedArgs) {
    eprintln!("# Searching for '{}'", resolved.search_word);
    if let Some((filelist, skipped_comments)) = &resolved.filelist {
        for comment in skipped_comments {
            eprintln!(
                "# Skipping comment on line {} of '{}': {}",
                comment.line,
                filelist.display(),
                comment.text
            );
        }
    }
}

/// Searches one file, turning a panic into that file's error so one bad file
/// reports why it failed instead of taking the whole run down with it.
fn search_file(
    filename: PathBuf,
    search_word: &str,
    algorithm: &dyn SearchAlgorithm,
    timing: bool,
) -> PerFileResult {
    panic::catch_unwind(AssertUnwindSafe(|| {
        run_file(&filename, search_word, algorithm, timing)
    }))
    .unwrap_or_else(|payload| PerFileResult::failed(filename, panic_message(&*payload)))
}

fn run_file(
    filename: &Path,
    search_word: &str,
    algorithm: &dyn SearchAlgorithm,
    timing: bool,
) -> PerFileResult {
    let loaded = match loader::load(filename) {
        Ok(l) => l,
        Err(e) => return PerFileResult::failed(filename.to_path_buf(), e.to_string()),
    };

    let binary = loader::looks_binary(loaded.as_bytes());

    // `elapsed` brackets the search only, not the load above, so it measures the
    // algorithm rather than the I/O. That is what makes bf and bmh comparable,
    // and what makes the summary's two throughput figures differ.
    let start = timing.then(Instant::now);
    let matches = algorithm.search(search_word, loaded.as_bytes());
    let timing = start.map(|start| TimingInfo {
        bytes: loaded.as_bytes().len() as u64,
        elapsed: start.elapsed(),
    });

    PerFileResult {
        filename: filename.to_path_buf(),
        matches,
        binary,
        timing,
        error: None,
    }
}

/// Megabytes per second for `bytes` transferred in `micros` microseconds, using
/// MB = 10^6 rather than 2^20 so the figure matches what disk and network tools
/// report.
///
/// The division looks like it is missing a conversion factor and is not: bytes
/// per microsecond *is* megabytes per second, since 10^6 µs make a second and
/// 10^6 bytes make a megabyte, and the two cancel.
///
/// Zero when no time elapsed. A run too small to measure has no meaningful rate,
/// and saying so beats dividing by zero.
// f64 represents integers exactly below 2^53, which is 9 petabytes of input and
// 285 years of elapsed time. Both operands are far under that, and the result is
// printed to one decimal place regardless.
#[allow(clippy::cast_precision_loss)]
fn megabytes_per_second(bytes: u64, micros: u128) -> f64 {
    if micros == 0 {
        return 0.0;
    }
    bytes as f64 / micros as f64
}

fn print_timing_summary(algorithm_code: &str, totals: &RunTotals, wall: Duration) {
    // `args_os`, not `args`: a non-UTF-8 *filename* reaches argv fine (paths are
    // PathBuf end to end), and `args()` would panic unwrapping it. `#COMMAND`
    // is an echo of what was invoked, and lossy display is exactly the right
    // fidelity for an echo — the same compromise the match output already makes.
    let command: Vec<String> = std::env::args_os()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    eprintln!("#COMMAND {}", command.join(" "));

    let files = totals.files;
    let errors = totals.errors;
    let matched = totals.files_matched;
    let lines = totals.matches;
    let wall_micros = wall.as_micros();

    // With no file searched, every per-file figure falls back to 0, and so do
    // both rates (`megabytes_per_second` gives 0 for no elapsed time).
    let micros = || totals.timings.iter().map(|t| t.elapsed.as_micros());
    let min = micros().min().unwrap_or(0);
    let max = micros().max().unwrap_or(0);
    let search_micros: u128 = micros().sum();
    let avg = search_micros
        .checked_div(totals.timings.len() as u128)
        .unwrap_or(0);
    let total_bytes: u64 = totals.timings.iter().map(|t| t.bytes).sum();

    // Two rates, because "throughput" is ambiguous while N threads are in flight
    // and picking one silently would hide a factor equal to the parallelism.
    //
    //   algo_mbps — bytes over the summed per-file search time. Excludes I/O and
    //               excludes parallelism, so it compares algorithms.
    //   wall_mbps — bytes over the elapsed run. What the user actually waited
    //               for, including loading and every thread running at once.
    let algo_mbps = megabytes_per_second(total_bytes, search_micros);
    let wall_mbps = megabytes_per_second(total_bytes, wall_micros);

    eprintln!(
        "#TIMING_SUMMARY algorithm={algorithm_code} files={files} errors={errors} \
         matched={matched} matches={lines} bytes={total_bytes} \
         min={min} avg={avg} max={max} \
         algo_mbps={algo_mbps:.1} wall_mbps={wall_mbps:.1} wall_us={wall_micros}"
    );
}

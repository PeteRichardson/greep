mod filelist;
mod loader;
mod options;
mod search;

use std::io::{BufWriter, Write};
use std::time::{Duration, Instant};

use clap::Parser;

use options::{resolve, Args};
use search::Match;

/// Exit codes follow grep: 0 = matched, 1 = no match, 2 = an error occurred.
/// An error outranks a match, so a run that both matched and failed reports 2.
const EXIT_MATCH: i32 = 0;
const EXIT_NO_MATCH: i32 = 1;
const EXIT_ERROR: i32 = 2;

struct PerFileResult {
    filename: String,
    matches: Vec<Match>,
    bytes: u64,
    elapsed: Option<Duration>,
    error: Option<String>,
}

/// What the timing summary still needs after a file's matches have been printed
/// and dropped. Keeping this instead of the whole `PerFileResult` is what lets
/// the match text be freed as soon as it has been written out.
struct FileStats {
    bytes: u64,
    elapsed: Option<Duration>,
    failed: bool,
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

    let Some(search_word) = args.search_word.clone() else {
        eprintln!("usage: greep [-v] [-t] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]");
        eprintln!("       greep -l");
        return EXIT_ERROR;
    };

    let resolved = match resolve(args, search_word) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_ERROR;
        }
    };

    if resolved.verbose {
        eprintln!("# Searching for '{}'", resolved.search_word);
    }

    let algorithm_code = resolved.algorithm_code.clone();
    let timing = resolved.timing;
    let verbose = resolved.verbose;
    let search_word = resolved.search_word.clone();

    let handles: Vec<_> = resolved
        .files
        .into_iter()
        .enumerate()
        .map(|(i, filename)| {
            if verbose {
                eprintln!("# Processing file {i}: {filename}");
            }
            let algorithm_code = algorithm_code.clone();
            let search_word = search_word.clone();
            // Kept outside the closure so a panicking worker can still be named.
            let name = filename.clone();
            let handle = std::thread::spawn(move || {
                run_file(&filename, &search_word, &algorithm_code, timing)
            });
            (name, handle)
        })
        .collect();

    // One lock and one buffer for the whole run, rather than a lock and a flush
    // per match.
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut stats = Vec::with_capacity(handles.len());
    let mut any_match = false;
    let mut any_error = false;

    // Join in spawn order and emit each file's matches as it lands, so the match
    // text is freed per file instead of accumulating until every thread is done.
    // Join order is argument order, which is what makes the output deterministic.
    for (name, handle) in handles {
        let result = match handle.join() {
            Ok(result) => result,
            Err(payload) => PerFileResult {
                filename: name,
                matches: Vec::new(),
                bytes: 0,
                elapsed: None,
                error: Some(panic_message(&*payload)),
            },
        };

        for m in &result.matches {
            let _ = writeln!(out, "{}:{} {}", result.filename, m.line_number, m.line);
        }
        any_match |= !result.matches.is_empty();

        if let Some(err) = &result.error {
            // Keep stdout and stderr readable relative to each other; this is
            // once per failing file, not once per match.
            let _ = out.flush();
            eprintln!("error: {}: {}", result.filename, err);
            any_error = true;
        }

        if timing {
            if let Some(elapsed) = result.elapsed {
                let _ = out.flush();
                eprintln!("#TIMING {:8} {}", elapsed.as_micros(), result.filename);
            }
        }

        stats.push(FileStats {
            bytes: result.bytes,
            elapsed: result.elapsed,
            failed: result.error.is_some(),
        });
        // `result`, and with it this file's match text, is dropped here.
    }

    if let Err(e) = out.flush() {
        eprintln!("error: writing to stdout: {e}");
        return EXIT_ERROR;
    }

    if timing {
        print_timing_summary(&algorithm_code, &stats);
    }

    if any_error {
        EXIT_ERROR
    } else if any_match {
        EXIT_MATCH
    } else {
        EXIT_NO_MATCH
    }
}

/// Recover the message from a panicking worker, so one bad file reports why it
/// failed instead of taking the whole process down with it.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        format!("worker thread panicked: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("worker thread panicked: {s}")
    } else {
        "worker thread panicked".to_string()
    }
}

fn run_file(
    filename: &str,
    search_word: &str,
    algorithm_code: &str,
    timing: bool,
) -> PerFileResult {
    let alg = search::find_algorithm(algorithm_code).expect("algorithm validated before spawn");

    let loaded = match loader::load(std::path::Path::new(filename)) {
        Ok(l) => l,
        Err(e) => {
            return PerFileResult {
                filename: filename.to_string(),
                matches: Vec::new(),
                bytes: 0,
                elapsed: None,
                error: Some(e.to_string()),
            };
        }
    };

    let bytes = loaded.as_bytes().len() as u64;

    let (matches, elapsed) = if timing {
        let start = Instant::now();
        let matches = alg.search(search_word, loaded.as_bytes());
        (matches, Some(start.elapsed()))
    } else {
        (alg.search(search_word, loaded.as_bytes()), None)
    };

    PerFileResult {
        filename: filename.to_string(),
        matches,
        bytes,
        elapsed,
        error: None,
    }
}

fn print_timing_summary(algorithm_code: &str, results: &[FileStats]) {
    let command: Vec<String> = std::env::args().collect();
    eprintln!("#COMMAND {}", command.join(" "));

    let files = results.len();
    let ok: Vec<&FileStats> = results.iter().filter(|r| !r.failed).collect();
    let errors = files - ok.len();

    if ok.is_empty() {
        eprintln!(
            "#TIMING_SUMMARY algorithm={algorithm_code} files={files} errors={errors} bytes=0 min=0 avg=0 max=0"
        );
        return;
    }

    let micros: Vec<u128> = ok.iter().map(|r| r.elapsed.unwrap().as_micros()).collect();
    let min = micros.iter().min().unwrap();
    let max = micros.iter().max().unwrap();
    let total: u128 = micros.iter().sum();
    let avg = total / ok.len() as u128;
    let total_bytes: u64 = ok.iter().map(|r| r.bytes).sum();

    eprintln!(
        "#TIMING_SUMMARY algorithm={algorithm_code} files={files} errors={errors} bytes={total_bytes} min={min} avg={avg} max={max}"
    );
}

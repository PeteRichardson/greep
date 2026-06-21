mod filelist;
mod loader;
mod options;
mod search;

use std::time::{Duration, Instant};

use clap::Parser;

use options::{resolve, Args};
use search::Match;

struct PerFileResult {
    filename: String,
    matches: Vec<Match>,
    bytes: u64,
    elapsed: Option<Duration>,
    error: Option<String>,
}

fn main() {
    let args = Args::parse();

    if args.list {
        options::print_algorithm_list();
        return;
    }

    let Some(search_word) = args.search_word.clone() else {
        eprintln!("usage: greep [-v] [-t] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]");
        eprintln!("       greep -l");
        std::process::exit(1);
    };

    let resolved = match resolve(args, search_word) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("# ERROR: {e}");
            std::process::exit(1);
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
            std::thread::spawn(move || run_file(&filename, &search_word, &algorithm_code, timing))
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.join().expect("worker thread panicked"));
    }

    for result in &results {
        for m in &result.matches {
            println!("{}:{} {}", result.filename, m.line_number, m.line);
        }
        if let Some(err) = &result.error {
            eprintln!("# ERROR: {}: {}", result.filename, err);
        }
        if timing {
            if let Some(elapsed) = result.elapsed {
                eprintln!("#TIMING {:8} {}", elapsed.as_micros(), result.filename);
            }
        }
    }

    if timing {
        print_timing_summary(&algorithm_code, &results);
    }
}

fn run_file(filename: &str, search_word: &str, algorithm_code: &str, timing: bool) -> PerFileResult {
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

fn print_timing_summary(algorithm_code: &str, results: &[PerFileResult]) {
    let command: Vec<String> = std::env::args().collect();
    eprintln!("#COMMAND {}", command.join(" "));

    let files = results.len();
    let ok: Vec<&PerFileResult> = results.iter().filter(|r| r.error.is_none()).collect();
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

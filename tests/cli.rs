use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use std::fs;
use tempfile::TempDir;

fn greep() -> Command {
    Command::cargo_bin("greep").unwrap()
}

/// A temp directory that removes itself when dropped, including while the thread
/// is unwinding from a failed assertion. The binding must be held for the whole
/// test — dropping it early deletes the fixture out from under the command.
fn fixture() -> TempDir {
    tempfile::tempdir().expect("create temp dir")
}

/// A path inside a fixture that is deliberately never created, for the tests that
/// need a path guaranteed not to exist.
fn missing_path(dir: &TempDir) -> String {
    dir.path()
        .join("does-not-exist.txt")
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn list_prints_known_codes() {
    greep()
        .arg("-l")
        .assert()
        .success()
        .stdout(predicates::str::contains("bf"))
        .stdout(predicates::str::contains("bmh"));
}

#[test]
fn unknown_algorithm_exits_nonzero() {
    greep()
        .args(["-a", "bogus", "word", "/dev/null"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown algorithm"));
}

#[test]
fn filelist_and_positional_files_conflict() {
    let dir = fixture();
    let list_path = dir.path().join("list.txt");
    fs::write(&list_path, "a.txt\n").unwrap();

    greep()
        .args(["-f", list_path.to_str().unwrap(), "word", "extra.txt"])
        .assert()
        .failure();
}

#[test]
fn filelist_duplicates_produce_one_result_not_one_per_line() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.txt");
    fs::write(&target, "needle here\n").unwrap();
    let list = dir.path().join("list.txt");
    // The reported symptom: N identical entries bought N threads, N reads of the
    // same file, and N copies of every matching line.
    let repeated = format!("{}\n", target.display()).repeat(5);
    fs::write(&list, &repeated).unwrap();

    let expected = format!("{}:1 needle here\n", target.display());

    greep()
        .args(["-f", list.to_str().unwrap(), "needle"])
        .assert()
        .code(0)
        .stdout(expected);
}

#[test]
fn filelist_comments_and_blank_lines_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.txt");
    fs::write(&target, "needle here\n").unwrap();
    let list = dir.path().join("list.txt");
    fs::write(
        &list,
        format!(
            "# this manifest is annotated\n\n{}\n   # and indented comments work too\n",
            target.display()
        ),
    )
    .unwrap();

    // A comment naming a nonexistent path would exit 2 if it were treated as one.
    greep()
        .args(["-f", list.to_str().unwrap(), "needle"])
        .assert()
        .code(0)
        .stdout(format!("{}:1 needle here\n", target.display()));
}

#[test]
fn directory_argument_expands_and_skips_dotfiles() {
    let dir = fixture();
    fs::write(dir.path().join("visible.txt"), "needle here\n").unwrap();
    fs::write(dir.path().join(".hidden.txt"), "needle here too\n").unwrap();

    greep()
        .args(["needle", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("visible.txt"))
        .stdout(predicates::str::contains(".hidden.txt").not());
}

#[test]
fn timing_summary_on_all_failed_files_is_zeroed_not_crashed() {
    let dir = fixture();

    greep()
        .args(["-t", "word", &missing_path(&dir)])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("errors=1"))
        .stderr(predicates::str::contains("min=0"))
        // The throughput fields take the same early return and must be zeroed
        // rather than dividing by a zero elapsed time.
        .stderr(predicates::str::contains("algo_mbps=0.0 wall_mbps=0.0"))
        .stderr(predicates::str::contains("matched=0 matches=0"));
}

#[test]
fn timing_summary_counts_matched_files_and_matching_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    // Two of three files match; three matching lines in total. Distinct numbers,
    // so a summary reporting one where it means the other cannot pass.
    fs::write(dir.join("a.txt"), "needle one\nplain\nneedle two\n").unwrap();
    fs::write(dir.join("b.txt"), "nothing here\n").unwrap();
    fs::write(dir.join("c.txt"), "needle three\n").unwrap();

    greep()
        .args([
            "-t",
            "needle",
            dir.join("a.txt").to_str().unwrap(),
            dir.join("b.txt").to_str().unwrap(),
            dir.join("c.txt").to_str().unwrap(),
        ])
        .assert()
        .code(0)
        .stderr(predicates::str::contains(
            "files=3 errors=0 matched=2 matches=3",
        ));
}

#[test]
fn matches_on_a_binary_file_are_counted_even_though_lines_are_not_printed() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let file = dir.join("blob.bin");
    let mut data = vec![0x7f, b'E', b'L', b'F', 0x00];
    data.extend_from_slice(b"needle\n");
    fs::write(&file, &data).unwrap();

    // The line itself is suppressed because it is arbitrary bytes, but it did
    // match, and the summary counts what was found rather than what was shown.
    greep()
        .args(["-t", "needle", file.to_str().unwrap()])
        .assert()
        .code(0)
        .stdout(predicates::str::contains("Binary file"))
        .stderr(predicates::str::contains("matched=1 matches=1"));
}

#[test]
fn timing_summary_reports_both_throughput_figures() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let file = dir.join("big.txt");
    // Large enough that the search takes a measurable number of microseconds;
    // on a small file both rates legitimately round to 0.0 and assert nothing.
    let mut body = "the quick brown fox jumps over the lazy dog\n".repeat(50_000);
    body.push_str("needle at the end\n");
    fs::write(&file, &body).unwrap();

    let output = greep()
        .args(["-t", "needle", file.to_str().unwrap()])
        .assert()
        .code(0)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).unwrap();

    let summary = stderr
        .lines()
        .find(|l| l.starts_with("#TIMING_SUMMARY"))
        .expect("a #TIMING_SUMMARY line");

    let field = |key: &str| -> f64 {
        summary
            .split_whitespace()
            .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("no {key} in {summary}"))
            .parse()
            .unwrap_or_else(|_| panic!("{key} is not a number in {summary}"))
    };

    // Both rates are real measurements, so assert the properties that must hold
    // rather than a magnitude that would be flaky on a loaded machine.
    assert!(field("algo_mbps") > 0.0, "algo_mbps was 0 in {summary}");
    assert!(field("wall_mbps") > 0.0, "wall_mbps was 0 in {summary}");
    assert!(field("wall_us") > 0.0, "wall_us was 0 in {summary}");

    // Wall clock covers loading and writing as well as searching, and this is a
    // single file so there is no parallelism to offset that. The search-only
    // rate is therefore the higher of the two.
    assert!(
        field("algo_mbps") > field("wall_mbps"),
        "expected search-only throughput to exceed wall-clock throughput in {summary}"
    );
}

#[test]
fn matches_found_exits_0() {
    let dir = fixture();
    let file = dir.path().join("a.txt");
    fs::write(&file, "needle here\n").unwrap();

    greep()
        .args(["needle", file.to_str().unwrap()])
        .assert()
        .code(0);
}

#[test]
fn no_matches_exits_1() {
    let dir = fixture();
    let file = dir.path().join("a.txt");
    fs::write(&file, "nothing interesting here\n").unwrap();

    greep()
        .args(["absent", file.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicates::str::is_empty());
}

#[test]
fn missing_file_exits_2_even_when_another_file_matches() {
    let dir = fixture();
    let good = dir.path().join("good.txt");
    fs::write(&good, "needle here\n").unwrap();

    // An error outranks a successful match: grep reports 2, not 0.
    greep()
        .args(["needle", good.to_str().unwrap(), &missing_path(&dir)])
        .assert()
        .code(2)
        .stdout(predicates::str::contains("good.txt"));
}

#[test]
fn per_file_errors_use_clap_style_prefix() {
    let dir = fixture();

    greep()
        .args(["word", &missing_path(&dir)])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("error:"))
        .stderr(predicates::str::contains("# ERROR:").not());
}

#[test]
fn multi_file_output_is_in_argument_order() {
    let dir = fixture();
    // Named so that argument order is the reverse of alphabetical order: if the
    // implementation ever sorted or emitted in completion order, this would catch it.
    let c = dir.path().join("c.txt");
    let b = dir.path().join("b.txt");
    let a = dir.path().join("a.txt");
    for p in [&c, &b, &a] {
        fs::write(p, "needle\n").unwrap();
    }

    let expected = format!(
        "{}:1 needle\n{}:1 needle\n{}:1 needle\n",
        c.display(),
        b.display(),
        a.display()
    );

    greep()
        .args([
            "needle",
            c.to_str().unwrap(),
            b.to_str().unwrap(),
            a.to_str().unwrap(),
        ])
        .assert()
        .code(0)
        .stdout(expected);
}

#[test]
fn help_documents_directory_walk_skipping() {
    // Issue #24: skipping symlinks matches `grep -r` and is correct; the defect
    // was that it happened silently. Lock the disclosure into --help so it
    // cannot be dropped without a failing test.
    greep()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("recursively"))
        .stdout(predicates::str::contains("symlink"))
        .stdout(predicates::str::contains("Dotfiles"));
}

#[test]
fn binary_file_with_a_match_reports_it_without_dumping_bytes() {
    let dir = fixture();
    let file = dir.path().join("blob.bin");
    // NUL in the first block, plus the search word further in.
    let mut data = vec![0x7f, b'E', b'L', b'F', 0x00, 0x01, 0x02, 0x03];
    data.extend_from_slice(b"\x01\x02needle\x03\x04");
    fs::write(&file, &data).unwrap();

    let expected = format!("Binary file {} matches\n", file.display());

    greep()
        .args(["needle", file.to_str().unwrap()])
        .assert()
        .code(0)
        .stdout(expected);
}

#[test]
fn binary_file_without_a_match_prints_nothing() {
    let dir = fixture();
    let file = dir.path().join("blob.bin");
    fs::write(&file, [0x7f, b'E', b'L', b'F', 0x00, 0x01, 0x02, 0x03]).unwrap();

    greep()
        .args(["needle", file.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicates::str::is_empty());
}

#[test]
fn empty_search_word_is_rejected() {
    // grep "" matches every line; greep silently matched nothing, which is the
    // worst of both. Reject it at the argument boundary instead.
    greep()
        .args(["", "/dev/null"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("empty"));
}

#[test]
fn missing_search_word_reports_clap_usage_not_a_hand_rolled_one() {
    greep()
        .assert()
        .code(2)
        // clap's generated usage, capital U...
        .stderr(predicates::str::contains("Usage:"))
        // ...and not the hand-rolled string that duplicated it.
        .stderr(predicates::str::contains("usage: greep [-v]").not());
}

#[test]
fn version_flag_prints_the_package_version() {
    greep()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn short_version_flag_does_not_collide_with_verbose() {
    // -v is verbose; clap's generated short for --version is -V. Both must work.
    greep()
        .arg("-V")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

// ---------------------------------------------------------------------------
// The stdin default path (#27)
// ---------------------------------------------------------------------------

#[test]
fn no_file_arguments_reads_stdin() {
    // With no positional files and no -f, `resolve` substitutes /dev/stdin. That
    // is the default path for every piped invocation and had no test at all.
    greep()
        .arg("needle")
        .write_stdin("first line\nneedle here\nlast line\n")
        .assert()
        .code(0)
        .stdout("/dev/stdin:2 needle here\n");
}

#[test]
fn stdin_with_no_match_exits_1() {
    greep()
        .arg("absent")
        .write_stdin("nothing interesting\n")
        .assert()
        .code(1)
        .stdout(predicates::str::is_empty());
}

#[test]
fn stdin_is_read_not_mmapped() {
    // A pipe is not a regular file, so `load` must take the read path however
    // large a size the fd claims. Getting output at all is the assertion: an
    // attempted mmap of a pipe fails, and the file would be reported as an error.
    let mut body = "filler line\n".repeat(5_000);
    body.push_str("needle here\n");

    greep()
        .arg("needle")
        .write_stdin(body)
        .assert()
        .code(0)
        .stdout("/dev/stdin:5001 needle here\n");
}

// ---------------------------------------------------------------------------
// -v and per-file #TIMING output (#29)
// ---------------------------------------------------------------------------

#[test]
fn verbose_names_the_search_word_and_every_file() {
    let dir = fixture();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "needle here\n").unwrap();
    fs::write(&b, "needle again\n").unwrap();

    greep()
        .args(["-v", "needle", a.to_str().unwrap(), b.to_str().unwrap()])
        .assert()
        .code(0)
        // Progress goes to stderr so stdout stays pipeable.
        .stderr(predicates::str::contains("# Searching for 'needle'"))
        .stderr(predicates::str::contains("# Processing file 0:"))
        .stderr(predicates::str::contains("# Processing file 1:"))
        .stdout(predicates::str::contains("needle here"))
        .stdout(predicates::str::contains("# Searching for").not());
}

#[test]
fn timing_prints_a_per_file_line_for_each_successful_file() {
    let dir = fixture();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "needle here\n").unwrap(); // 12 bytes
    fs::write(&b, "needle again\n").unwrap(); // 13 bytes

    greep()
        .args(["-t", "needle", a.to_str().unwrap(), b.to_str().unwrap()])
        .assert()
        .code(0)
        .stderr(predicates::str::contains("#TIMING "))
        .stderr(predicates::str::contains(a.to_str().unwrap()))
        .stderr(predicates::str::contains(b.to_str().unwrap()))
        .stderr(predicates::str::contains("#COMMAND"))
        // 12 + 13. Pins that `bytes` is a real sum over the files searched rather
        // than, say, a file count. `matched`/`matches` sit between `errors` and
        // `bytes` as of #50, and are included so this stays one contiguous run.
        .stderr(predicates::str::contains(
            "files=2 errors=0 matched=2 matches=2 bytes=25",
        ));
}

#[test]
fn timing_omits_the_per_file_line_for_a_file_that_failed() {
    let dir = fixture();
    let good = dir.path().join("good.txt");
    fs::write(&good, "needle here\n").unwrap();
    let missing = missing_path(&dir);

    // A failed file has no `elapsed`, so it counts toward files= and errors= but
    // gets no #TIMING line of its own, and contributes no bytes.
    greep()
        .args(["-t", "needle", good.to_str().unwrap(), &missing])
        .assert()
        .code(2)
        .stderr(predicates::str::contains(
            "files=2 errors=1 matched=1 matches=1 bytes=12",
        ))
        .stderr(predicates::str::contains(format!("#TIMING {missing}")).not());
}

#[test]
fn timing_and_verbose_are_independent() {
    let dir = fixture();
    let file = dir.path().join("a.txt");
    fs::write(&file, "needle here\n").unwrap();

    // -t alone: timing output, no progress lines.
    greep()
        .args(["-t", "needle", file.to_str().unwrap()])
        .assert()
        .code(0)
        .stderr(predicates::str::contains("#TIMING_SUMMARY"))
        .stderr(predicates::str::contains("# Searching for").not());

    // -v alone: progress lines, no timing output.
    greep()
        .args(["-v", "needle", file.to_str().unwrap()])
        .assert()
        .code(0)
        .stderr(predicates::str::contains("# Searching for 'needle'"))
        .stderr(predicates::str::contains("#TIMING").not());
}

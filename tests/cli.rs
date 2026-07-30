use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use std::fs;

fn greep() -> Command {
    Command::cargo_bin("greep").unwrap()
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
    let dir = std::env::temp_dir().join(format!("greep-cli-test-conflict-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let list_path = dir.join("list.txt");
    fs::write(&list_path, "a.txt\n").unwrap();

    greep()
        .args(["-f", list_path.to_str().unwrap(), "word", "extra.txt"])
        .assert()
        .failure();

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn directory_argument_expands_and_skips_dotfiles() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-dir-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("visible.txt"), "needle here\n").unwrap();
    fs::write(dir.join(".hidden.txt"), "needle here too\n").unwrap();

    greep()
        .args(["needle", dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("visible.txt"))
        .stdout(predicates::str::contains(".hidden.txt").not());

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn timing_summary_on_all_failed_files_is_zeroed_not_crashed() {
    let missing =
        std::env::temp_dir().join(format!("greep-cli-test-missing-{}", std::process::id()));

    greep()
        .args(["-t", "word", missing.to_str().unwrap()])
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
    let dir = std::env::temp_dir().join(format!("greep-cli-test-counts-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
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

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn matches_on_a_binary_file_are_counted_even_though_lines_are_not_printed() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-bincount-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
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

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn timing_summary_reports_both_throughput_figures() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-mbps-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
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

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn matches_found_exits_0() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-found-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.txt");
    fs::write(&file, "needle here\n").unwrap();

    greep()
        .args(["needle", file.to_str().unwrap()])
        .assert()
        .code(0);

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn no_matches_exits_1() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-nomatch-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.txt");
    fs::write(&file, "nothing interesting here\n").unwrap();

    greep()
        .args(["absent", file.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicates::str::is_empty());

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn missing_file_exits_2_even_when_another_file_matches() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-mixed-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let good = dir.join("good.txt");
    fs::write(&good, "needle here\n").unwrap();
    let missing = dir.join("nope.txt");

    // An error outranks a successful match: grep reports 2, not 0.
    greep()
        .args(["needle", good.to_str().unwrap(), missing.to_str().unwrap()])
        .assert()
        .code(2)
        .stdout(predicates::str::contains("good.txt"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn per_file_errors_use_clap_style_prefix() {
    let missing =
        std::env::temp_dir().join(format!("greep-cli-test-prefix-{}", std::process::id()));

    greep()
        .args(["word", missing.to_str().unwrap()])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("error:"))
        .stderr(predicates::str::contains("# ERROR:").not());
}

#[test]
fn multi_file_output_is_in_argument_order() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-order-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    // Named so that argument order is the reverse of alphabetical order: if the
    // implementation ever sorted or emitted in completion order, this would catch it.
    let c = dir.join("c.txt");
    let b = dir.join("b.txt");
    let a = dir.join("a.txt");
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

    fs::remove_dir_all(&dir).unwrap();
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
    let dir = std::env::temp_dir().join(format!("greep-cli-test-bin-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("blob.bin");
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

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn binary_file_without_a_match_prints_nothing() {
    let dir = std::env::temp_dir().join(format!("greep-cli-test-binnm-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("blob.bin");
    fs::write(&file, [0x7f, b'E', b'L', b'F', 0x00, 0x01, 0x02, 0x03]).unwrap();

    greep()
        .args(["needle", file.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicates::str::is_empty());

    fs::remove_dir_all(&dir).unwrap();
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

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
        .stderr(predicates::str::contains("min=0"));
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

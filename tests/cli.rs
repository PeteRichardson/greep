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
    let missing = std::env::temp_dir().join(format!("greep-cli-test-missing-{}", std::process::id()));

    greep()
        .args(["-t", "word", missing.to_str().unwrap()])
        .assert()
        .success()
        .stderr(predicates::str::contains("errors=1"))
        .stderr(predicates::str::contains("min=0"));
}

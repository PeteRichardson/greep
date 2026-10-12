use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Expands a leading `~` to the value of `$HOME`.
///
/// A manifest is read by greep rather than by a shell, so nothing else in the
/// pipeline will ever expand it — a `~/notes.txt` line would otherwise become a
/// request to open a directory literally named `~`.
///
/// `~user` is deliberately not supported: resolving another user's home means a
/// passwd lookup, and a path beginning `~someone` is far more likely to be a real
/// relative path than an unsupported shorthand. Only a bare `~` or a `~/` prefix
/// is treated as the shorthand.
fn expand_tilde(path: &str) -> PathBuf {
    // `var_os`, not `var`: `$HOME` names a path, and a path is not required to be
    // UTF-8. `var` reports a non-UTF-8 home as absent and would silently stop
    // expanding for a user who has one.
    expand_tilde_against(path, std::env::var_os("HOME").as_deref())
}

/// The body of [`expand_tilde`], with `$HOME` passed in.
///
/// Taking the home directory as an argument keeps the separator and
/// missing-`$HOME` rules testable without mutating the environment, which is
/// process-global and races the other tests in this module.
fn expand_tilde_against(path: &str, home: Option<&OsStr>) -> PathBuf {
    let rest = if path == "~" {
        ""
    } else if let Some(rest) = path.strip_prefix("~/") {
        // `Path::push` treats a leading separator as "this is absolute" and
        // discards what came before it, so `~//notes.txt` would resolve to
        // `/notes.txt` and drop `$HOME` entirely.
        rest.trim_start_matches('/')
    } else {
        return PathBuf::from(path);
    };

    let Some(home) = home else {
        // No `$HOME` to expand against. Passing the path through unchanged fails
        // later with a "no such file" naming the path the user actually wrote,
        // which beats failing here with something they did not.
        return PathBuf::from(path);
    };

    // `Path::push` owns the separator rules, so a `$HOME` written with or without
    // a trailing slash produces the same result and there is no concatenation
    // here to get them wrong.
    let mut expanded = PathBuf::from(home);
    if !rest.is_empty() {
        // Pushing an empty component would append a separator, turning a bare
        // `~` into `$HOME/`.
        expanded.push(rest);
    }
    expanded
}

/// Reads a manifest of paths, one per line.
///
/// Blank lines are skipped, `#` comments are ignored, `~` is expanded, and
/// repeated paths are collapsed to their first occurrence — a duplicated line
/// otherwise buys a second search, a second read of the same file, and a second
/// copy of every matching line in the output.
pub fn read_filelist(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim_end_matches(['\r', '\n']);

        // A comment is a line whose first non-blank character is `#`. Only the
        // whole line — a trailing `# ...` is not stripped, because `#` is a legal
        // character in a filename and `notes#2.txt` must stay openable. A file
        // whose name genuinely starts with `#` can still be listed as `./#name`.
        if trimmed.is_empty() || trimmed.trim_start().starts_with('#') {
            continue;
        }

        // Leading whitespace is *not* stripped from a path: it is legal in a
        // filename, and this function has no way to tell alignment from content.
        let expanded = expand_tilde(trimmed);
        if seen.insert(expanded.clone()) {
            out.push(expanded);
        }
    }
    Ok(out)
}

/// One step of a walk: a file to search, or something the walk could not read.
///
/// A failed `read_dir` used to print an error and vanish: it happened on the
/// main thread before any worker existed, so it never reached the per-file error
/// channel and the run exited 1 (no match) rather than 2 (error). Carrying it in
/// the walk's own output puts it in sequence with the files around it, for the
/// caller to report and count. The same goes for an entry inside a directory
/// that cannot be read: a walk that quietly drops one is an incomplete search
/// that reports itself as complete.
#[derive(Debug, PartialEq, Eq)]
pub enum WalkItem {
    File(PathBuf),
    Error(String),
}

/// What the walk does with one directory entry.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Descend(PathBuf),
    Yield(WalkItem),
    /// Skipped on purpose, not because of an error.
    Skip,
}

/// Decides what to do with the entry at `path`, given its type.
///
/// Symlinks and special files are skipped, as `grep -r` does (#24). A type
/// that cannot be read is reported, never skipped, ENOENT included (#63): an
/// entry deleted mid-walk is the same race as a file deleted before a worker
/// opens it, and that one is already reported.
fn classify_type(path: PathBuf, file_type: io::Result<fs::FileType>) -> Step {
    match file_type {
        Ok(ft) if ft.is_dir() => Step::Descend(path),
        Ok(ft) if ft.is_file() => Step::Yield(WalkItem::File(path)),
        Ok(_) => Step::Skip,
        Err(e) => Step::Yield(WalkItem::Error(format!(
            "unable to read the type of '{}': {e}",
            path.display()
        ))),
    }
}

/// Decides what to do with one entry read from `dir`.
///
/// An entry that cannot be read at all is reported against `dir`, since there
/// is no name to report it against. On macOS and Linux, std's `ReadDir` ends
/// after such an error, so a broken directory costs one error, not an endless
/// stream of them. std does not document this, so it holds only in practice.
fn classify_entry(dir: &Path, entry: io::Result<fs::DirEntry>) -> Step {
    let entry = match entry {
        Ok(entry) => entry,
        Err(e) => {
            return Step::Yield(WalkItem::Error(format!(
                "unable to read directory '{}': {e}",
                dir.display()
            )))
        }
    };
    // Tested on the raw bytes rather than through `to_string_lossy`, so a name
    // that is not valid UTF-8 is classified on what it actually starts with
    // instead of on a decoded copy of itself.
    if entry.file_name().as_encoded_bytes().starts_with(b".") {
        return Step::Skip;
    }
    classify_type(entry.path(), entry.file_type())
}

/// Expands directory arguments into the files they contain, lazily.
///
/// A path that is not a directory, including one that does not exist, is yielded
/// as given; opening it is the worker's job, and so is reporting its error.
///
/// Lazy so that the search can start on the first file while the rest of the tree
/// is still being walked. Walking everything up front was a serial prelude
/// before any worker began.
pub fn walk(paths: Vec<PathBuf>) -> Walk {
    Walk {
        args: paths.into_iter(),
        open_dirs: Vec::new(),
    }
}

pub struct Walk {
    args: std::vec::IntoIter<PathBuf>,
    /// The directories being read, innermost last. Finishing the innermost one
    /// before resuming its parent gives the same depth-first, `read_dir` order a
    /// recursive walk does. Each keeps its path, to name it in an error.
    open_dirs: Vec<(PathBuf, fs::ReadDir)>,
}

impl Walk {
    /// Starts reading `dir`, or reports that it cannot be read.
    fn enter(&mut self, dir: &Path) -> Option<WalkItem> {
        match fs::read_dir(dir) {
            Ok(entries) => {
                self.open_dirs.push((dir.to_path_buf(), entries));
                None
            }
            // One item per failed directory, so a run's error count and exit
            // status see it. The search proceeds with what *could* be opened.
            // The OS reason is kept: "permission denied" and "too many open
            // files" need different fixes, and the message is all the user gets.
            Err(e) => Some(WalkItem::Error(format!(
                "unable to open directory '{}': {e}",
                dir.display()
            ))),
        }
    }
}

impl Iterator for Walk {
    type Item = WalkItem;

    fn next(&mut self) -> Option<WalkItem> {
        loop {
            if let Some((dir, entries)) = self.open_dirs.last_mut() {
                let Some(entry) = entries.next() else {
                    self.open_dirs.pop();
                    continue;
                };
                match classify_entry(dir, entry) {
                    Step::Descend(child) => {
                        if let Some(err) = self.enter(&child) {
                            return Some(err);
                        }
                    }
                    Step::Yield(item) => return Some(item),
                    Step::Skip => {}
                }
                continue;
            }

            let path = self.args.next()?;
            match fs::metadata(&path) {
                Ok(meta) if meta.is_dir() => {
                    if let Some(err) = self.enter(&path) {
                        return Some(err);
                    }
                }
                _ => return Some(WalkItem::File(path)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A temp directory that removes itself when dropped, including while the
    /// thread is unwinding from a failed assertion. The explicit `remove_*` calls
    /// this replaces were skipped on failure, leaking fixtures into the temp dir.
    fn fixture() -> tempfile::TempDir {
        tempfile::tempdir().expect("create temp dir")
    }

    /// Writes `contents` to a manifest file and reads it back through
    /// `read_filelist`. The `TempDir` cleans up on drop, so neither a failed
    /// write nor a failed assertion in the caller can leak the fixture.
    fn read_manifest(name: &str, contents: &str) -> Vec<PathBuf> {
        let dir = fixture();
        let path = dir.path().join(name);
        fs::write(&path, contents).unwrap();
        read_filelist(&path).unwrap()
    }

    #[test]
    fn read_filelist_skips_blank_lines_and_trims() {
        let dir = fixture();
        let path = dir.path().join("list.txt");
        {
            let mut f = fs::File::create(&path).unwrap();
            writeln!(f, "first.txt").unwrap();
            writeln!(f).unwrap();
            writeln!(f, "second.txt\r").unwrap();
        }
        let result = read_filelist(&path).unwrap();
        assert_eq!(
            result,
            vec![PathBuf::from("first.txt"), PathBuf::from("second.txt")]
        );
    }

    #[test]
    fn read_filelist_dedupes_preserving_first_occurrence_order() {
        // Order is first-seen, not sorted and not last-seen: "b" stays ahead of
        // "c" because its first mention was.
        let result = read_manifest("dupes.txt", "b.txt\nc.txt\nb.txt\na.txt\nc.txt\nb.txt\n");
        assert_eq!(
            result,
            vec![
                PathBuf::from("b.txt"),
                PathBuf::from("c.txt"),
                PathBuf::from("a.txt")
            ]
        );
    }

    #[test]
    fn read_filelist_skips_comment_lines() {
        let result = read_manifest(
            "comments.txt",
            "# a leading comment\nfirst.txt\n   # an indented one\nsecond.txt\n",
        );
        assert_eq!(
            result,
            vec![PathBuf::from("first.txt"), PathBuf::from("second.txt")]
        );
    }

    #[test]
    fn read_filelist_does_not_strip_trailing_comments_from_paths() {
        // `#` is legal in a filename. Stripping from the first `#` onwards would
        // silently rewrite these two paths into files that do not exist.
        let result = read_manifest("hashes.txt", "notes#2.txt\n./#literal.txt\n");
        assert_eq!(
            result,
            vec![
                PathBuf::from("notes#2.txt"),
                PathBuf::from("./#literal.txt")
            ]
        );
    }

    #[test]
    fn read_filelist_expands_a_leading_tilde() {
        let home = std::env::var("HOME").expect("HOME is set");
        let result = read_manifest("tilde.txt", "~/notes.txt\n~\n");
        assert_eq!(
            result,
            vec![
                PathBuf::from(format!("{home}/notes.txt")),
                PathBuf::from(home)
            ]
        );
    }

    #[test]
    fn read_filelist_leaves_other_tildes_alone() {
        // `~user` needs a passwd lookup and is not supported; an interior `~` is
        // an ordinary character. Neither may be rewritten.
        let result = read_manifest("tilde2.txt", "~someone/notes.txt\nback~up.txt\n./~odd\n");
        assert_eq!(
            result,
            vec![
                PathBuf::from("~someone/notes.txt"),
                PathBuf::from("back~up.txt"),
                PathBuf::from("./~odd")
            ]
        );
    }

    #[test]
    fn read_filelist_dedupes_after_tilde_expansion() {
        // `~/x` and `$HOME/x` are the same file, so they must collapse. Deduping
        // before expansion would leave both.
        let home = std::env::var("HOME").expect("HOME is set");
        let result = read_manifest("tilde3.txt", &format!("~/notes.txt\n{home}/notes.txt\n"));
        assert_eq!(result, vec![PathBuf::from(format!("{home}/notes.txt"))]);
    }

    /// `$HOME` written with a trailing separator must expand identically to one
    /// written without. The concatenation this replaced needed a
    /// `trim_end_matches` to get here; `Path::push` does it as a matter of course.
    #[test]
    fn expand_tilde_is_indifferent_to_a_trailing_slash_on_home() {
        let plain = expand_tilde_against("~/notes.txt", Some(OsStr::new("/home/pete")));
        let trailing = expand_tilde_against("~/notes.txt", Some(OsStr::new("/home/pete/")));
        assert_eq!(plain, PathBuf::from("/home/pete/notes.txt"));
        assert_eq!(trailing, plain);
    }

    #[test]
    fn expand_tilde_keeps_home_when_the_remainder_starts_with_a_separator() {
        // `Path::push` would treat `/notes.txt` as absolute and throw `$HOME`
        // away, which is the one way this can silently name the wrong file.
        let result = expand_tilde_against("~//notes.txt", Some(OsStr::new("/home/pete")));
        assert_eq!(result, PathBuf::from("/home/pete/notes.txt"));
    }

    #[test]
    fn expand_tilde_of_a_bare_tilde_has_no_trailing_separator() {
        let result = expand_tilde_against("~", Some(OsStr::new("/home/pete")));
        assert_eq!(result, PathBuf::from("/home/pete"));
    }

    #[test]
    fn expand_tilde_passes_the_path_through_when_home_is_unset() {
        // The fallback the environment cannot be made to exercise directly: an
        // unexpanded `~/notes.txt` fails later naming what the user actually
        // wrote, rather than failing here naming something they did not.
        assert_eq!(
            expand_tilde_against("~/notes.txt", None),
            PathBuf::from("~/notes.txt")
        );
        assert_eq!(expand_tilde_against("~", None), PathBuf::from("~"));
    }

    #[test]
    fn expand_tilde_leaves_non_tilde_paths_untouched_regardless_of_home() {
        for path in ["notes.txt", "~someone/notes.txt", "back~up.txt", "./~odd"] {
            assert_eq!(
                expand_tilde_against(path, Some(OsStr::new("/home/pete"))),
                PathBuf::from(path)
            );
        }
    }

    /// Runs a whole walk and splits it into its files and its errors, which is
    /// the shape most of these tests want to assert on.
    fn expand_paths(paths: Vec<PathBuf>) -> (Vec<PathBuf>, Vec<String>) {
        let mut files = Vec::new();
        let mut errors = Vec::new();
        for item in walk(paths) {
            match item {
                WalkItem::File(f) => files.push(f),
                WalkItem::Error(e) => errors.push(e),
            }
        }
        (files, errors)
    }

    /// Argument order is output order, so a directory's files must come out
    /// between the arguments either side of it, not before or after them all.
    #[test]
    fn walk_keeps_each_directory_in_its_argument_position() {
        let tmp = fixture();
        let dir = tmp.path();
        fs::create_dir(dir.join("middle")).unwrap();
        fs::write(dir.join("middle").join("m.txt"), b"m").unwrap();
        let first = dir.join("z-first.txt");
        let last = dir.join("a-last.txt");

        let items: Vec<_> = walk(vec![first.clone(), dir.join("middle"), last.clone()]).collect();
        assert_eq!(
            items,
            vec![
                WalkItem::File(first),
                WalkItem::File(dir.join("middle").join("m.txt")),
                WalkItem::File(last),
            ]
        );
    }

    /// A nested directory is finished before its parent's later entries, as in
    /// a recursive walk. The parent here holds only the one subdirectory and one
    /// file, so either `read_dir` order gives a sequence this can check.
    #[test]
    fn walk_is_depth_first() {
        let tmp = fixture();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("sub").join("deeper")).unwrap();
        fs::write(dir.join("sub").join("deeper").join("d.txt"), b"d").unwrap();
        fs::write(dir.join("sub").join("s.txt"), b"s").unwrap();
        fs::write(dir.join("top.txt"), b"t").unwrap();

        let (files, errors) = expand_paths(vec![dir.to_path_buf()]);
        assert!(errors.is_empty(), "no walk errors expected");
        let top = files.iter().position(|f| f.ends_with("top.txt")).unwrap();
        let sub: Vec<usize> = files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.starts_with(dir.join("sub")))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(files.len(), 3);
        assert_eq!(sub.len(), 2);
        // The two files under `sub` are adjacent: nothing from the parent
        // interrupts the subtree.
        assert_eq!(sub[1], sub[0] + 1);
        assert!(top < sub[0] || top > sub[1]);
    }

    #[test]
    fn expand_paths_passes_through_regular_files_and_unstatable_paths() {
        let (result, errors) = expand_paths(vec![PathBuf::from("/dev/stdin")]);
        assert_eq!(result, vec![PathBuf::from("/dev/stdin")]);
        assert!(errors.is_empty(), "no walk errors expected");
    }

    /// An unreadable directory reports an error through the channel the caller
    /// turns into exit status 2, instead of being skipped silently.
    #[cfg(unix)]
    #[test]
    fn expand_paths_reports_an_unwalkable_directory() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = fixture();
        let dir = tmp.path();
        let locked = dir.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("x.txt"), b"x").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        // Root reads through a 000 mode, so on such a system there is nothing
        // to exercise and this returns early — the same convention the
        // non-UTF-8 name test uses for APFS.
        let still_readable = fs::read_dir(&locked).is_ok();
        let (result, errors) = expand_paths(vec![locked.clone()]);
        // Restore before any assert can fail, so the TempDir can delete x.txt.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if still_readable {
            return;
        }

        assert!(result.is_empty(), "a locked directory yields no files");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("unable to open directory"));
        // The OS reason, so a locked directory can be told apart from, say,
        // running out of file descriptors on a deep tree.
        assert!(
            errors[0].contains("Permission denied"),
            "no OS reason in {:?}",
            errors[0]
        );
    }

    #[test]
    fn expand_paths_walks_directory_skipping_dotfiles() {
        let tmp = fixture();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), b"a").unwrap();
        fs::write(dir.join(".hidden"), b"h").unwrap();
        fs::write(dir.join("sub").join("b.txt"), b"b").unwrap();
        fs::create_dir_all(dir.join(".hiddendir")).unwrap();
        fs::write(dir.join(".hiddendir").join("c.txt"), b"c").unwrap();

        let (mut result, errors) = expand_paths(vec![dir.to_path_buf()]);
        assert!(errors.is_empty(), "no walk errors expected");
        result.sort();

        let mut expected = vec![dir.join("a.txt"), dir.join("sub").join("b.txt")];
        expected.sort();

        assert_eq!(result, expected);
    }

    /// Symlinks and special files are skipped on purpose (#24), as `grep -r`
    /// does. Only an *error* reading an entry is reported; these are not errors.
    #[cfg(unix)]
    #[test]
    fn walk_skips_symlinks_and_special_files_without_an_error() {
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let tmp = fixture();
        let dir = tmp.path();
        fs::write(dir.join("real.txt"), b"r").unwrap();
        fs::create_dir(dir.join("realdir")).unwrap();
        symlink(dir.join("real.txt"), dir.join("link-to-file")).unwrap();
        symlink(dir.join("realdir"), dir.join("link-to-dir")).unwrap();
        symlink(dir.join("nowhere"), dir.join("dangling")).unwrap();
        let _socket = UnixListener::bind(dir.join("sock")).unwrap();

        let (files, errors) = expand_paths(vec![dir.to_path_buf()]);
        assert_eq!(files, vec![dir.join("real.txt")]);
        assert!(errors.is_empty(), "skips are not errors: {errors:?}");
    }

    /// No real filesystem greep runs on makes `file_type()` fail on demand, so
    /// the error is handed in directly. ENOENT gets no special case (#63): a
    /// file deleted mid-walk is reported, as it is when a worker fails to open
    /// one.
    #[test]
    fn an_entry_whose_type_cannot_be_read_is_reported_not_skipped() {
        let path = PathBuf::from("dir/gone.txt");
        let gone = io::Error::new(io::ErrorKind::NotFound, "deleted mid-walk");
        let step = classify_type(path, Err(gone));
        let Step::Yield(WalkItem::Error(msg)) = step else {
            panic!("expected a reported error, got {step:?}");
        };
        assert!(msg.contains("'dir/gone.txt'"), "no path in {msg:?}");
        assert!(msg.contains("deleted mid-walk"), "no OS reason in {msg:?}");
    }

    /// A `ReadDir` fails mid-directory only on a failing disk or mount, which
    /// no test can arrange, so the error is handed in directly.
    #[test]
    fn an_entry_that_cannot_be_read_is_reported_not_skipped() {
        let step = classify_entry(Path::new("some/dir"), Err(io::Error::other("disk on fire")));
        let Step::Yield(WalkItem::Error(msg)) = step else {
            panic!("expected a reported error, got {step:?}");
        };
        assert!(msg.contains("'some/dir'"), "no directory in {msg:?}");
        assert!(msg.contains("disk on fire"), "no OS reason in {msg:?}");
    }

    #[cfg(unix)]
    #[test]
    fn walk_preserves_non_utf8_names_so_they_stay_openable() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let tmp = fixture();
        let dir = tmp.path();

        // 0xFF is never valid UTF-8. APFS rejects such a name outright, so on
        // macOS there is nothing to exercise and this returns early; ext4 accepts
        // it, so CI runs the real case.
        let target = dir.join(OsStr::from_bytes(b"caf\xff.txt"));
        if fs::write(&target, b"needle\n").is_err() {
            return;
        }

        let (result, errors) = expand_paths(vec![dir.to_path_buf()]);
        assert!(errors.is_empty(), "no walk errors expected");
        assert_eq!(result.len(), 1);

        // The bytes survive the walk, so the path still names the file it came
        // from and still opens.
        assert_eq!(
            result[0].as_os_str().as_bytes(),
            target.as_os_str().as_bytes()
        );
        assert!(fs::File::open(&result[0]).is_ok());

        // And this is the bug being fixed, demonstrated rather than described:
        // routing the same path through a lossy String turns 0xFF into U+FFFD and
        // produces something that no longer opens.
        let lossy = PathBuf::from(result[0].to_string_lossy().into_owned());
        assert!(
            fs::File::open(&lossy).is_err(),
            "the lossy form opened, so this test is not exercising the mangling"
        );
    }
}

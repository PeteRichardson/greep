use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
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
fn expand_tilde(path: &str) -> String {
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
fn expand_tilde_against(path: &str, home: Option<&OsStr>) -> String {
    let rest = if path == "~" {
        ""
    } else if let Some(rest) = path.strip_prefix("~/") {
        // `Path::push` treats a leading separator as "this is absolute" and
        // discards what came before it, so `~//notes.txt` would resolve to
        // `/notes.txt` and drop `$HOME` entirely.
        rest.trim_start_matches('/')
    } else {
        return path.to_string();
    };

    let Some(home) = home else {
        // No `$HOME` to expand against. Passing the path through unchanged fails
        // later with a "no such file" naming the path the user actually wrote,
        // which beats failing here with something they did not.
        return path.to_string();
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

    // This function still yields `String` because `read_filelist` does. A
    // non-UTF-8 result therefore has to fall back to the unexpanded path rather
    // than be mangled by a lossy conversion — issue #39 tracks carrying
    // `PathBuf` end to end, which retires both this branch and the return type.
    expanded
        .into_os_string()
        .into_string()
        .unwrap_or_else(|_| path.to_string())
}

/// Reads a manifest of paths, one per line.
///
/// Blank lines are skipped, `#` comments are ignored, `~` is expanded, and
/// repeated paths are collapsed to their first occurrence — a duplicated line
/// otherwise buys a second thread, a second read of the same file, and a second
/// copy of every matching line in the output.
pub fn read_filelist(path: &Path) -> std::io::Result<Vec<String>> {
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

pub fn expand_paths(paths: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for path in paths {
        match fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => walk_directory(Path::new(&path), &mut out),
            _ => out.push(path),
        }
    }
    out
}

fn walk_directory(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        eprintln!("error: unable to open directory '{}'", dir.display());
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let child: PathBuf = entry.path();
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => walk_directory(&child, out),
            Ok(ft) if ft.is_file() => out.push(child.to_string_lossy().into_owned()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "greep-filelist-test-{}-{}",
            name,
            std::process::id()
        ))
    }

    /// Writes `contents` to a manifest file and reads it back through
    /// `read_filelist`, cleaning up afterwards.
    fn read_manifest(name: &str, contents: &str) -> Vec<String> {
        let path = unique_dir(name);
        fs::write(&path, contents).unwrap();
        let result = read_filelist(&path);
        fs::remove_file(&path).unwrap();
        result.unwrap()
    }

    #[test]
    fn read_filelist_skips_blank_lines_and_trims() {
        let path = unique_dir("list.txt");
        {
            let mut f = fs::File::create(&path).unwrap();
            writeln!(f, "first.txt").unwrap();
            writeln!(f).unwrap();
            writeln!(f, "second.txt\r").unwrap();
        }
        let result = read_filelist(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(
            result,
            vec!["first.txt".to_string(), "second.txt".to_string()]
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
                "b.txt".to_string(),
                "c.txt".to_string(),
                "a.txt".to_string()
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
            vec!["first.txt".to_string(), "second.txt".to_string()]
        );
    }

    #[test]
    fn read_filelist_does_not_strip_trailing_comments_from_paths() {
        // `#` is legal in a filename. Stripping from the first `#` onwards would
        // silently rewrite these two paths into files that do not exist.
        let result = read_manifest("hashes.txt", "notes#2.txt\n./#literal.txt\n");
        assert_eq!(
            result,
            vec!["notes#2.txt".to_string(), "./#literal.txt".to_string()]
        );
    }

    #[test]
    fn read_filelist_expands_a_leading_tilde() {
        let home = std::env::var("HOME").expect("HOME is set");
        let result = read_manifest("tilde.txt", "~/notes.txt\n~\n");
        assert_eq!(result, vec![format!("{home}/notes.txt"), home]);
    }

    #[test]
    fn read_filelist_leaves_other_tildes_alone() {
        // `~user` needs a passwd lookup and is not supported; an interior `~` is
        // an ordinary character. Neither may be rewritten.
        let result = read_manifest("tilde2.txt", "~someone/notes.txt\nback~up.txt\n./~odd\n");
        assert_eq!(
            result,
            vec![
                "~someone/notes.txt".to_string(),
                "back~up.txt".to_string(),
                "./~odd".to_string()
            ]
        );
    }

    #[test]
    fn read_filelist_dedupes_after_tilde_expansion() {
        // `~/x` and `$HOME/x` are the same file, so they must collapse. Deduping
        // before expansion would leave both.
        let home = std::env::var("HOME").expect("HOME is set");
        let result = read_manifest("tilde3.txt", &format!("~/notes.txt\n{home}/notes.txt\n"));
        assert_eq!(result, vec![format!("{home}/notes.txt")]);
    }

    /// `$HOME` written with a trailing separator must expand identically to one
    /// written without. The concatenation this replaced needed a
    /// `trim_end_matches` to get here; `Path::push` does it as a matter of course.
    #[test]
    fn expand_tilde_is_indifferent_to_a_trailing_slash_on_home() {
        let plain = expand_tilde_against("~/notes.txt", Some(OsStr::new("/home/pete")));
        let trailing = expand_tilde_against("~/notes.txt", Some(OsStr::new("/home/pete/")));
        assert_eq!(plain, "/home/pete/notes.txt");
        assert_eq!(trailing, plain);
    }

    #[test]
    fn expand_tilde_keeps_home_when_the_remainder_starts_with_a_separator() {
        // `Path::push` would treat `/notes.txt` as absolute and throw `$HOME`
        // away, which is the one way this can silently name the wrong file.
        let result = expand_tilde_against("~//notes.txt", Some(OsStr::new("/home/pete")));
        assert_eq!(result, "/home/pete/notes.txt");
    }

    #[test]
    fn expand_tilde_of_a_bare_tilde_has_no_trailing_separator() {
        let result = expand_tilde_against("~", Some(OsStr::new("/home/pete")));
        assert_eq!(result, "/home/pete");
    }

    #[test]
    fn expand_tilde_passes_the_path_through_when_home_is_unset() {
        // The fallback the environment cannot be made to exercise directly: an
        // unexpanded `~/notes.txt` fails later naming what the user actually
        // wrote, rather than failing here naming something they did not.
        assert_eq!(expand_tilde_against("~/notes.txt", None), "~/notes.txt");
        assert_eq!(expand_tilde_against("~", None), "~");
    }

    #[test]
    fn expand_tilde_leaves_non_tilde_paths_untouched_regardless_of_home() {
        for path in ["notes.txt", "~someone/notes.txt", "back~up.txt", "./~odd"] {
            assert_eq!(
                expand_tilde_against(path, Some(OsStr::new("/home/pete"))),
                path
            );
        }
    }

    #[test]
    fn expand_paths_passes_through_regular_files_and_unstatable_paths() {
        let result = expand_paths(vec!["/dev/stdin".to_string()]);
        assert_eq!(result, vec!["/dev/stdin".to_string()]);
    }

    #[test]
    fn expand_paths_walks_directory_skipping_dotfiles() {
        let dir = unique_dir("walkdir");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), b"a").unwrap();
        fs::write(dir.join(".hidden"), b"h").unwrap();
        fs::write(dir.join("sub").join("b.txt"), b"b").unwrap();
        fs::create_dir_all(dir.join(".hiddendir")).unwrap();
        fs::write(dir.join(".hiddendir").join("c.txt"), b"c").unwrap();

        let mut result = expand_paths(vec![dir.to_string_lossy().into_owned()]);
        result.sort();

        let mut expected = vec![
            dir.join("a.txt").to_string_lossy().into_owned(),
            dir.join("sub").join("b.txt").to_string_lossy().into_owned(),
        ];
        expected.sort();

        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(result, expected);
    }
}

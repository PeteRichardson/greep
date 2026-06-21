use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub fn read_filelist(path: &Path) -> std::io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
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
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("# ERROR: unable to open directory '{}'", dir.display());
            return;
        }
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
        std::env::temp_dir().join(format!("greep-filelist-test-{}-{}", name, std::process::id()))
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
        assert_eq!(result, vec!["first.txt".to_string(), "second.txt".to_string()]);
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

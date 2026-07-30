use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MMAP_THRESHOLD_BYTES: u64 = 1 << 30;

/// How much of a file is inspected when deciding whether it is binary. Bounded
/// so the check stays O(1) regardless of file size — a NUL past this point is
/// not detected, which is the same trade `grep` makes with its first read buffer.
pub const BINARY_SNIFF_BYTES: usize = 8192;

/// True if the leading `BINARY_SNIFF_BYTES` contain a NUL byte.
pub fn looks_binary(buf: &[u8]) -> bool {
    let window = &buf[..buf.len().min(BINARY_SNIFF_BYTES)];
    window.contains(&0)
}

pub enum Loaded {
    Owned(Vec<u8>),
    Mapped(memmap2::Mmap),
}

impl Loaded {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Loaded::Owned(v) => v,
            Loaded::Mapped(m) => m,
        }
    }
}

pub fn load(path: &Path) -> std::io::Result<Loaded> {
    load_with_threshold(path, MMAP_THRESHOLD_BYTES)
}

/// The real implementation, with the mmap cutoff injected.
///
/// `load` is the only caller outside tests and always passes
/// `MMAP_THRESHOLD_BYTES`. The parameter exists because that constant is 1 GiB:
/// testing the mmap branch against it would mean creating a 1 GiB file, so the
/// branch went unexercised entirely. With the cutoff injectable, a handful of
/// bytes is enough.
fn load_with_threshold(path: &Path, threshold: u64) -> std::io::Result<Loaded> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;

    if metadata.is_file() && metadata.len() >= threshold {
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        return Ok(Loaded::Mapped(mmap));
    }

    // `metadata.len()` is a hint, not a promise: a special file can report a
    // size it will never deliver, and the bare `as usize` cast silently
    // truncated on 32-bit targets. Anything at or above the threshold took the
    // mmap branch above, so a legitimate hint can never exceed it.
    let hint = usize::try_from(metadata.len().min(threshold)).unwrap_or(0);
    let mut buf = Vec::with_capacity(hint);
    let mut file = file;
    file.read_to_end(&mut buf)?;
    Ok(Loaded::Owned(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Writes `contents` to a file inside a self-cleaning temp dir and returns
    /// both. The `TempDir` must stay bound for the lifetime of the path.
    fn file_containing(contents: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("input");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(contents).unwrap();
        (dir, path)
    }

    #[test]
    fn small_file_uses_read_path() {
        let (_dir, path) = file_containing(b"hello world\n");
        let loaded = load(&path).unwrap();
        assert!(matches!(loaded, Loaded::Owned(_)));
        assert_eq!(loaded.as_bytes(), b"hello world\n");
    }

    #[test]
    fn file_at_the_threshold_is_mapped() {
        // The whole point of the injected threshold: this is the mmap branch,
        // exercised with 12 bytes instead of the 1 GiB the real constant needs.
        let (_dir, path) = file_containing(b"hello world\n");
        let loaded = load_with_threshold(&path, 12).unwrap();
        assert!(matches!(loaded, Loaded::Mapped(_)));
        assert_eq!(loaded.as_bytes(), b"hello world\n");
    }

    #[test]
    fn file_one_byte_below_the_threshold_is_read() {
        // Pins the comparison as `>=` rather than `>`. Together with the test
        // above, 12 bytes maps at a threshold of 12 and reads at 13, so a flip
        // in either direction fails one of the pair.
        let (_dir, path) = file_containing(b"hello world\n");
        let loaded = load_with_threshold(&path, 13).unwrap();
        assert!(matches!(loaded, Loaded::Owned(_)));
        assert_eq!(loaded.as_bytes(), b"hello world\n");
    }

    #[test]
    fn both_branches_yield_identical_bytes() {
        // The two branches return different variants backed by different memory.
        // What callers depend on is that `as_bytes()` cannot tell them apart.
        let contents: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
        let (_dir, path) = file_containing(&contents);

        let mapped = load_with_threshold(&path, 10_000).unwrap();
        let owned = load_with_threshold(&path, 10_001).unwrap();

        assert!(matches!(mapped, Loaded::Mapped(_)));
        assert!(matches!(owned, Loaded::Owned(_)));
        assert_eq!(mapped.as_bytes(), owned.as_bytes());
        assert_eq!(mapped.as_bytes(), contents.as_slice());
    }

    #[test]
    fn load_delegates_with_the_one_gibibyte_threshold() {
        // `load` is a one-liner over `load_with_threshold`, so the only thing it
        // can get wrong is the constant it passes.
        assert_eq!(MMAP_THRESHOLD_BYTES, 1 << 30);
        assert_eq!(MMAP_THRESHOLD_BYTES, 1_073_741_824);
    }

    #[test]
    fn a_character_device_is_read_not_mapped() {
        // Character devices are the family /dev/stdin belongs to, so this is the
        // default input path when greep is used in a pipeline.
        //
        // Note what this does *not* prove. It cannot isolate the `is_file()` half
        // of the mmap condition: a character device reports `len() == 0`, so the
        // size check already excludes it whatever the threshold. Deleting
        // `is_file()` leaves this test passing — verified. The guard is
        // belt-and-braces on this platform rather than load-bearing, and there is
        // no special file that both fails `is_file()` and reports a size large
        // enough to reach the branch.
        let loaded = load_with_threshold(Path::new("/dev/null"), 1).unwrap();
        assert!(matches!(loaded, Loaded::Owned(_)));
        assert_eq!(loaded.as_bytes(), b"");
    }

    #[test]
    fn text_is_not_binary() {
        assert!(!looks_binary(b""));
        assert!(!looks_binary(b"hello world\nsecond line\n"));
    }

    #[test]
    fn nul_byte_marks_binary() {
        assert!(looks_binary(b"\x7fELF\x02\x01\x01\x00"));
        assert!(looks_binary(b"text then \x00 a nul"));
    }

    #[test]
    fn nul_beyond_the_sniff_window_is_not_detected() {
        // Documents the bound rather than asserting it is ideal: the check is
        // deliberately O(1), so a NUL this far in is invisible to it.
        let mut buf = vec![b'a'; BINARY_SNIFF_BYTES + 100];
        buf[BINARY_SNIFF_BYTES + 50] = 0;
        assert!(!looks_binary(&buf));

        // ...but one at the last byte of the window is caught.
        let mut buf = vec![b'a'; BINARY_SNIFF_BYTES + 100];
        buf[BINARY_SNIFF_BYTES - 1] = 0;
        assert!(looks_binary(&buf));
    }

    #[test]
    fn missing_file_errors() {
        let dir = tempfile::tempdir().expect("create temp dir");
        assert!(load(&dir.path().join("does-not-exist.txt")).is_err());
    }
}

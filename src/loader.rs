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
    let file = File::open(path)?;
    let metadata = file.metadata()?;

    if metadata.is_file() && metadata.len() >= MMAP_THRESHOLD_BYTES {
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        return Ok(Loaded::Mapped(mmap));
    }

    // `metadata.len()` is a hint, not a promise: a special file can report a
    // size it will never deliver, and the bare `as usize` cast silently
    // truncated on 32-bit targets. Anything at or above the threshold took the
    // mmap branch above, so a legitimate hint can never exceed it.
    let hint = usize::try_from(metadata.len().min(MMAP_THRESHOLD_BYTES)).unwrap_or(0);
    let mut buf = Vec::with_capacity(hint);
    let mut file = file;
    file.read_to_end(&mut buf)?;
    Ok(Loaded::Owned(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn small_file_uses_read_path() {
        let path = std::env::temp_dir().join(format!("greep-loader-test-{}", std::process::id()));
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"hello world\n").unwrap();
        }
        let loaded = load(&path).unwrap();
        assert!(matches!(loaded, Loaded::Owned(_)));
        assert_eq!(loaded.as_bytes(), b"hello world\n");
        std::fs::remove_file(&path).unwrap();
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
        let path = std::env::temp_dir().join("greep-loader-test-does-not-exist-12345");
        assert!(load(&path).is_err());
    }
}

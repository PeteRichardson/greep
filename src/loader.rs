use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const MMAP_THRESHOLD_BYTES: u64 = 1 << 30;

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
    fn missing_file_errors() {
        let path = std::env::temp_dir().join("greep-loader-test-does-not-exist-12345");
        assert!(load(&path).is_err());
    }
}

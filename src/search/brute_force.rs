use super::{Match, SearchAlgorithm};

#[derive(Debug)]
pub struct Bf;

impl SearchAlgorithm for Bf {
    fn search(&self, word: &str, buf: &[u8]) -> Vec<Match> {
        let word = word.as_bytes();
        if word.is_empty() {
            return Vec::new();
        }

        // Splitting on the terminator replaces the hand-rolled line-bounds scan
        // and its two index variables, which tracked the same value as each
        // other. Line numbers come from the enumeration rather than a counter.
        //
        // A buffer ending in '\n' makes `split` yield a trailing empty slice.
        // That costs nothing and needs no special case: an empty line cannot
        // contain a non-empty word, so it never becomes a match, and it is last,
        // so it cannot shift the numbering of anything before it.
        buf.split(|&b| b == b'\n')
            .enumerate()
            .filter(|(_, line)| line.windows(word.len()).any(|w| w == word))
            .map(|(index, line)| Match {
                line_number: index as u64 + 1,
                line: String::from_utf8_lossy(line).into_owned(),
            })
            .collect()
    }
}

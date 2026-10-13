use super::{Match, SearchAlgorithm};

#[derive(Debug)]
pub struct Bmh;

impl SearchAlgorithm for Bmh {
    // The newline count below is vectorized by the compiler; `bytecount` would
    // be one more dependency for a small gain.
    #[allow(clippy::naive_bytecount)]
    fn search(&self, word: &str, buf: &[u8]) -> Vec<Match> {
        let word = word.as_bytes();
        let word_len = word.len();
        // A matching span is byte-for-byte equal to `word`, so it crosses a line
        // boundary exactly when `word` itself contains a '\n'. Search is
        // line-scoped, so such a word can never match — reject it once here
        // rather than re-checking every candidate.
        if word_len == 0 || buf.len() < word_len || word.contains(&b'\n') {
            return Vec::new();
        }

        let mut shift = [word_len; 256];
        for (i, &b) in word[..word_len - 1].iter().enumerate() {
            shift[b as usize] = word_len - 1 - i;
        }

        let mut matches = Vec::new();
        // `line_number` and `line_start` describe the line that starts at
        // `line_start`. They are brought up to date only when a match is found,
        // so the bytes the skip table jumps over are counted in one pass per
        // match, not looked at one by one as the window moves.
        let mut line_number: u64 = 1;
        let mut line_start = 0usize;
        let mut win_start = 0usize;

        while win_start + word_len <= buf.len() {
            if buf[win_start..win_start + word_len] == *word {
                let skipped = &buf[line_start..win_start];
                line_number += skipped.iter().filter(|&&b| b == b'\n').count() as u64;
                line_start = skipped
                    .iter()
                    .rposition(|&b| b == b'\n')
                    .map_or(line_start, |p| line_start + p + 1);

                let line_end = buf[win_start + word_len..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(buf.len(), |p| win_start + word_len + p);
                matches.push(Match {
                    line_number,
                    line: String::from_utf8_lossy(&buf[line_start..line_end]).into_owned(),
                });

                win_start = (line_end + 1).min(buf.len());
                line_number += 1;
                line_start = win_start;
            } else {
                let bad_char = buf[win_start + word_len - 1] as usize;
                win_start += shift[bad_char];
            }
        }

        matches
    }
}

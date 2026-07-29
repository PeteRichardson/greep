use super::{Match, SearchAlgorithm};

pub struct Bmh;

impl SearchAlgorithm for Bmh {
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
        let mut line_number: u64 = 1;
        let mut line_start = 0usize;
        let mut scan_pos = 0usize;
        let mut win_start = 0usize;

        while win_start + word_len <= buf.len() {
            while scan_pos < win_start {
                if buf[scan_pos] == b'\n' {
                    line_number += 1;
                    line_start = scan_pos + 1;
                }
                scan_pos += 1;
            }

            let mut j = word_len - 1;
            let is_match = loop {
                if buf[win_start + j] != word[j] {
                    break false;
                }
                if j == 0 {
                    break true;
                }
                j -= 1;
            };

            if is_match {
                let mut line_end = win_start + word_len;
                while line_end < buf.len() && buf[line_end] != b'\n' {
                    line_end += 1;
                }
                matches.push(Match {
                    line_number,
                    line: String::from_utf8_lossy(&buf[line_start..line_end]).into_owned(),
                });

                win_start = if line_end < buf.len() {
                    line_end + 1
                } else {
                    buf.len()
                };
                scan_pos = win_start;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_matches_same_line_reports_first_only() {
        let bmh = Bmh;
        let result = bmh.search("aa", b"aaaa\n");
        assert_eq!(
            result,
            vec![Match {
                line_number: 1,
                line: "aaaa".to_string()
            }]
        );
    }
}

use super::{Match, SearchAlgorithm};

pub struct Bf;

impl SearchAlgorithm for Bf {
    fn search(&self, word: &str, buf: &[u8]) -> Vec<Match> {
        let word = word.as_bytes();
        if word.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();
        let mut line_number: u64 = 1;
        let mut line_start = 0usize;
        let mut pos = 0usize;

        while pos < buf.len() {
            // Find the next line's bounds.
            let mut line_end = line_start;
            while line_end < buf.len() && buf[line_end] != b'\n' {
                line_end += 1;
            }
            let line = &buf[line_start..line_end];

            if find_first(line, word).is_some() {
                matches.push(Match {
                    line_number,
                    line: String::from_utf8_lossy(line).into_owned(),
                });
            }

            line_number += 1;
            pos = if line_end < buf.len() { line_end + 1 } else { buf.len() };
            line_start = pos;
        }

        matches
    }
}

fn find_first(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod bf_only_tests {
    use super::*;

    #[test]
    fn finds_first_match_per_line() {
        let bf = Bf;
        let result = bf.search("needle", b"hay needle stack\nneedle needle\nno match here\n");
        assert_eq!(
            result,
            vec![
                Match { line_number: 1, line: "hay needle stack".to_string() },
                Match { line_number: 2, line: "needle needle".to_string() },
            ]
        );
    }

    #[test]
    fn no_match() {
        let bf = Bf;
        assert_eq!(bf.search("missing", b"nothing to see\n"), vec![]);
    }

    #[test]
    fn word_longer_than_buffer() {
        let bf = Bf;
        assert_eq!(bf.search("toolong", b"hi"), vec![]);
    }

    #[test]
    fn match_on_last_line_no_trailing_newline() {
        let bf = Bf;
        let result = bf.search("end", b"line one\nline two has end");
        assert_eq!(
            result,
            vec![Match { line_number: 2, line: "line two has end".to_string() }]
        );
    }
}

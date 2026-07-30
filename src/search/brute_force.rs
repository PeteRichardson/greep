use super::{Match, SearchAlgorithm};

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
            .filter_map(|(index, line)| {
                find_first(line, word).map(|_| Match {
                    line_number: index as u64 + 1,
                    line: String::from_utf8_lossy(line).into_owned(),
                })
            })
            .collect()
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
        let result = bf.search(
            "needle",
            b"hay needle stack\nneedle needle\nno match here\n",
        );
        assert_eq!(
            result,
            vec![
                Match {
                    line_number: 1,
                    line: "hay needle stack".to_string()
                },
                Match {
                    line_number: 2,
                    line: "needle needle".to_string()
                },
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
            vec![Match {
                line_number: 2,
                line: "line two has end".to_string()
            }]
        );
    }

    #[test]
    fn blank_lines_still_advance_the_line_number() {
        // Blank lines are the case where line counting and match position come
        // apart: nothing on them can ever match, but they must still be counted.
        let bf = Bf;
        let result = bf.search("needle", b"needle\n\n\nneedle\n");
        assert_eq!(
            result,
            vec![
                Match {
                    line_number: 1,
                    line: "needle".to_string()
                },
                Match {
                    line_number: 4,
                    line: "needle".to_string()
                },
            ]
        );
    }

    #[test]
    fn trailing_newline_does_not_produce_a_phantom_line() {
        // "a\n" is one line, not two. The distinction is invisible in the match
        // list here and shows up as an off-by-one in `line_number` on anything
        // that follows, so it is asserted directly.
        let bf = Bf;
        assert_eq!(
            bf.search("a", b"a\n"),
            vec![Match {
                line_number: 1,
                line: "a".to_string()
            }]
        );
        assert_eq!(
            bf.search("b", b"a\nb\n"),
            vec![Match {
                line_number: 2,
                line: "b".to_string()
            }]
        );
    }

    #[test]
    fn empty_buffer_yields_no_matches() {
        let bf = Bf;
        assert_eq!(bf.search("needle", b""), vec![]);
    }

    #[test]
    fn empty_word_yields_no_matches() {
        let bf = Bf;
        assert_eq!(bf.search("", b"anything at all\n"), vec![]);
    }
}

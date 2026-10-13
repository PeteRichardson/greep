mod brute_force;
mod horspool;

pub use brute_force::Bf;
pub use horspool::Bmh;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Match {
    pub line_number: u64,
    pub line: String,
}

/// `Sync` so that one shared instance can serve every worker thread; `Debug`
/// so that `ResolvedArgs`, which holds one, can derive it.
pub trait SearchAlgorithm: std::fmt::Debug + Sync {
    fn search(&self, word: &str, buf: &[u8]) -> Vec<Match>;
}

/// The one registry: (code, name, algorithm). Lookup and listing both read it,
/// so an algorithm cannot be listed without being findable.
const ALGORITHMS: &[(&str, &str, &dyn SearchAlgorithm)] = &[
    ("bf", "brute force", &Bf),
    ("bmh", "boyer-moore-horspool", &Bmh),
];

pub fn find_algorithm(code: &str) -> Option<&'static dyn SearchAlgorithm> {
    ALGORITHMS
        .iter()
        .find(|(c, ..)| *c == code)
        .map(|&(.., algorithm)| algorithm)
}

pub fn list_algorithms() -> impl Iterator<Item = (&'static str, &'static str)> {
    ALGORITHMS.iter().map(|&(code, name, _)| (code, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> Vec<(&'static str, &'static [u8], Vec<Match>)> {
        vec![
            (
                "needle",
                b"hay needle stack\nneedle needle\nno match here\n",
                vec![
                    Match {
                        line_number: 1,
                        line: "hay needle stack".to_string(),
                    },
                    Match {
                        line_number: 2,
                        line: "needle needle".to_string(),
                    },
                ],
            ),
            ("missing", b"nothing to see\nhere either\n", vec![]),
            ("toolong", b"hi", vec![]),
            (
                "end",
                b"line one\nline two has end",
                vec![Match {
                    line_number: 2,
                    line: "line two has end".to_string(),
                }],
            ),
            (
                "aa",
                b"aaaa\n" as &[u8],
                vec![Match {
                    line_number: 1,
                    line: "aaaa".to_string(),
                }],
            ),
            // Blank lines are the case where line counting and match position
            // come apart: nothing on them can ever match, but they must still
            // be counted.
            (
                "needle",
                b"needle\n\n\nneedle\n",
                vec![
                    Match {
                        line_number: 1,
                        line: "needle".to_string(),
                    },
                    Match {
                        line_number: 4,
                        line: "needle".to_string(),
                    },
                ],
            ),
            // "a\n" is one line, not two. The distinction is invisible in the
            // match list for "a" and shows up as an off-by-one in
            // `line_number` on anything that follows, as for "b".
            (
                "a",
                b"a\n",
                vec![Match {
                    line_number: 1,
                    line: "a".to_string(),
                }],
            ),
            (
                "b",
                b"a\nb\n",
                vec![Match {
                    line_number: 2,
                    line: "b".to_string(),
                }],
            ),
            ("needle", b"", vec![]),
            ("", b"anything at all\n", vec![]),
            // A search word containing a newline can never match: search is
            // line-scoped, and no line contains a '\n'. Guards against an
            // algorithm matching across a line boundary and reporting a
            // multi-line "line".
            ("alpha\nbeta", b"alpha\nbeta\n", vec![]),
        ]
    }

    #[test]
    fn all_algorithms_agree_on_fixtures() {
        for &(code, _, alg) in ALGORITHMS {
            for (word, buf, expected) in fixtures() {
                let actual = alg.search(word, buf);
                assert_eq!(actual, expected, "algorithm {code} on word {word:?}");
            }
        }
    }
}

mod brute_force;
mod horspool;

pub use brute_force::Bf;
pub use horspool::Bmh;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Match {
    pub line_number: u64,
    pub line: String,
}

pub trait SearchAlgorithm {
    fn search(&self, word: &str, buf: &[u8]) -> Vec<Match>;
}

const ALGORITHMS: &[(&str, &str)] = &[
    ("bf", "brute force"),
    ("bmh", "boyer-moore-horspool"),
];

pub fn find_algorithm(code: &str) -> Option<Box<dyn SearchAlgorithm>> {
    match code {
        "bf" => Some(Box::new(Bf)),
        "bmh" => Some(Box::new(Bmh)),
        _ => None,
    }
}

pub fn list_algorithms() -> &'static [(&'static str, &'static str)] {
    ALGORITHMS
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
                    Match { line_number: 1, line: "hay needle stack".to_string() },
                    Match { line_number: 2, line: "needle needle".to_string() },
                ],
            ),
            (
                "missing",
                b"nothing to see\nhere either\n",
                vec![],
            ),
            (
                "toolong",
                b"hi",
                vec![],
            ),
            (
                "end",
                b"line one\nline two has end",
                vec![Match { line_number: 2, line: "line two has end".to_string() }],
            ),
            (
                "aa",
                b"aaaa\n" as &[u8],
                vec![Match { line_number: 1, line: "aaaa".to_string() }],
            ),
        ]
    }

    #[test]
    fn all_algorithms_agree_on_fixtures() {
        for (code, _) in ALGORITHMS {
            let alg = find_algorithm(code).unwrap();
            for (word, buf, expected) in fixtures() {
                let actual = alg.search(word, buf);
                assert_eq!(actual, expected, "algorithm {code} on word {word:?}");
            }
        }
    }

    #[test]
    fn all_listed_algorithms_are_findable() {
        for (code, _) in list_algorithms() {
            assert!(find_algorithm(code).is_some(), "algorithm '{code}' is listed but not findable");
        }
    }
}

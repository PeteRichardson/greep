use super::{Match, SearchAlgorithm};

pub struct Bmh;

impl SearchAlgorithm for Bmh {
    fn search(&self, _word: &str, _buf: &[u8]) -> Vec<Match> {
        unimplemented!("implemented in Task 3")
    }
}

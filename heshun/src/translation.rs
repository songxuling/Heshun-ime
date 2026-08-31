//! 轻量候选 Translation 管线，参考 librime 的 Translation/Merged/Distinct。

use crate::engine::Candidate;
use std::collections::HashSet;

pub trait Translation {
    fn peek(&self) -> Option<&Candidate>;
    fn next(&mut self) -> bool;
    fn exhausted(&self) -> bool;
}

pub struct FifoTranslation {
    items: Vec<Candidate>,
    index: usize,
}

impl FifoTranslation {
    pub fn new(items: Vec<Candidate>) -> Self { Self { items, index: 0 } }
}

impl Translation for FifoTranslation {
    fn peek(&self) -> Option<&Candidate> { self.items.get(self.index) }
    fn next(&mut self) -> bool {
        if self.index >= self.items.len() { return false; }
        self.index += 1;
        self.index < self.items.len()
    }
    fn exhausted(&self) -> bool { self.index >= self.items.len() }
}

pub fn distinct(candidates: impl IntoIterator<Item = Candidate>, limit: usize) -> Vec<Candidate> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.word.clone()) {
            output.push(candidate);
            if limit != 0 && output.len() >= limit { break; }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CandidateSource;

    fn candidate(word: &str, source: CandidateSource) -> Candidate {
        Candidate { word: word.into(), code: "a".into(), source }
    }

    #[test]
    fn distinct_removes_duplicates_across_sources() {
        let items = distinct(vec![
            candidate("中", CandidateSource::ScriptExact),
            candidate("中", CandidateSource::ScriptSentence),
            candidate("国", CandidateSource::ScriptExact),
        ], 0);
        assert_eq!(items.iter().map(|c| c.word.as_str()).collect::<Vec<_>>(), vec!["中", "国"]);
    }
}

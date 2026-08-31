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

/// Rime MergedTranslation 的轻量实现：按来源质量交错消费，并跨来源去重。
pub struct MergedTranslation {
    sources: Vec<FifoTranslation>,
}

impl MergedTranslation {
    pub fn new(sources: Vec<FifoTranslation>) -> Self { Self { sources } }

    pub fn collect(mut self, limit: usize) -> Vec<Candidate> {
        let mut output = Vec::new();
        loop {
            let mut best: Option<(usize, &Candidate)> = None;
            for (index, source) in self.sources.iter().enumerate() {
                if let Some(candidate) = source.peek() {
                    let replace = best.map(|(best_index, best_candidate)| {
                        source_rank(candidate) < source_rank(best_candidate)
                            || (source_rank(candidate) == source_rank(best_candidate)
                                && index < best_index)
                    }).unwrap_or(true);
                    if replace { best = Some((index, candidate)); }
                }
            }
            let Some((index, candidate)) = best else { break; };
            let candidate = candidate.clone();
            let _ = self.sources[index].next();
            if output.iter().all(|item: &Candidate| item.word != candidate.word) {
                output.push(candidate);
                if limit != 0 && output.len() >= limit { break; }
            }
        }
        output
    }
}

fn source_rank(candidate: &Candidate) -> u8 {
    match candidate.source {
        crate::core::CandidateSource::Table | crate::core::CandidateSource::ScriptExact => 0,
        crate::core::CandidateSource::ScriptSentence => 1,
        crate::core::CandidateSource::TableCompletion
        | crate::core::CandidateSource::ScriptPrefix
        | crate::core::CandidateSource::ScriptAbbreviation => 2,
        crate::core::CandidateSource::ScriptCorrection => 3,
        crate::core::CandidateSource::Reverse => 4,
    }
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

    #[test]
    fn merged_translation_prefers_exact_source_and_deduplicates() {
        let merged = MergedTranslation::new(vec![
            FifoTranslation::new(vec![candidate("中国", CandidateSource::ScriptSentence)]),
            FifoTranslation::new(vec![candidate("中国", CandidateSource::ScriptExact), candidate("中", CandidateSource::ScriptExact)]),
        ]);
        let items = merged.collect(0);
        assert_eq!(items.iter().map(|c| c.word.as_str()).collect::<Vec<_>>(), vec!["中国", "中"]);
        assert_eq!(items[0].source, CandidateSource::ScriptExact);
    }
}

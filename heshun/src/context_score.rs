//! 上下文评分：可选的前文/二元词转移权重。

use crate::scorer::{BasicScorer, CandidateScorer};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BigramScorer {
    pub base: BasicScorer,
    transitions: HashMap<(String, String), f64>,
    pub unknown_transition: f64,
}

impl BigramScorer {
    pub fn new(base: BasicScorer) -> Self {
        Self { base, transitions: HashMap::new(), unknown_transition: 0.0 }
    }

    pub fn insert(&mut self, previous: impl Into<String>, current: impl Into<String>, score: f64) {
        self.transitions.insert((previous.into(), current.into()), score);
    }
}

impl CandidateScorer for BigramScorer {
    fn score_word(&self, previous_word: Option<&str>, word: &str, weight: u32, is_sentence_end: bool) -> f64 {
        self.base.score_word(previous_word, word, weight, is_sentence_end)
            + previous_word
                .and_then(|previous| self.transitions.get(&(previous.to_owned(), word.to_owned())))
                .copied()
                .unwrap_or(self.unknown_transition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_bigram_changes_score() {
        let base = BasicScorer::default();
        let mut scorer = BigramScorer::new(base);
        scorer.insert("中", "国", 2.0);
        let plain = base.score_word(Some("中"), "国", 90, false);
        assert_eq!(scorer.score_word(Some("中"), "国", 90, false), plain + 2.0);
    }
}

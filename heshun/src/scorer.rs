//! 候选评分：从词频累加升级到可替换的 Poet 风格评分接口。

use crate::segmentation::SpellingType;

pub trait CandidateScorer {
    fn score_word(&self, previous_word: Option<&str>, word: &str, weight: u32, is_sentence_end: bool) -> f64;
}

#[derive(Debug, Clone, Copy)]
pub struct BasicScorer {
    pub word_count_penalty: f64,
    pub length_bonus: f64,
    pub sentence_end_bonus: f64,
}

impl Default for BasicScorer {
    fn default() -> Self {
        Self { word_count_penalty: 0.15, length_bonus: 0.08, sentence_end_bonus: 0.0 }
    }
}

impl CandidateScorer for BasicScorer {
    fn score_word(&self, _previous_word: Option<&str>, word: &str, weight: u32, is_sentence_end: bool) -> f64 {
        (weight as f64 + 1.0).ln()
            + self.length_bonus * word.chars().count() as f64
            - self.word_count_penalty
            + if is_sentence_end { self.sentence_end_bonus } else { 0.0 }
    }
}

pub fn spelling_penalty(kind: SpellingType) -> f64 {
    match kind {
        SpellingType::Normal => 0.0,
        SpellingType::Abbreviation => -2.3,
        SpellingType::Completion => -3.0,
        // Rime's kCorrectionPenalty = log(0.01).
        SpellingType::Correction => -4.605_170_185_988_091,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_score_is_log_weight_and_length_aware() {
        let scorer = BasicScorer::default();
        assert!(scorer.score_word(None, "中国", 100, false) > scorer.score_word(None, "中", 100, false));
    }

    #[test]
    fn spelling_penalties_keep_normal_first() {
        assert_eq!(spelling_penalty(SpellingType::Normal), 0.0);
        assert!(spelling_penalty(SpellingType::Completion) < spelling_penalty(SpellingType::Abbreviation));
        assert!((spelling_penalty(SpellingType::Correction) - 0.01_f64.ln()).abs() < 1e-12);
    }
}

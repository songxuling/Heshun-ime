//! 词图与 beam search 组句，参考 librime 的 Poet::WordGraph。

use crate::pinyin::PinyinDict;
use crate::scorer::{BasicScorer, CandidateScorer};
use crate::segmentation::SyllableGraph;
use crate::user_dict::UserDict;

#[derive(Debug, Clone, PartialEq)]
pub struct WordEdge {
    pub start: usize,
    pub end: usize,
    pub word: String,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedSentence {
    pub words: Vec<String>,
    pub score: f64,
}

const SENTENCE_CUTOFF: f64 = 8.0;

pub fn build_word_graph(input: &str, dict: &PinyinDict, per_code_limit: usize) -> Vec<WordEdge> {
    let graph = SyllableGraph::build(input, dict);
    let mut edges = Vec::new();
    for edge in graph.edges {
        for candidate in dict.exact_limited(&edge.code, per_code_limit) {
            edges.push(WordEdge {
                start: edge.start,
                end: edge.end,
                word: candidate.word,
                weight: candidate.weight,
            });
        }
    }
    edges
}

/// Build a graph where learned user phrases are additional edges.
pub fn build_word_graph_with_user(
    input: &str,
    dict: &PinyinDict,
    user_dict: Option<&UserDict>,
    per_code_limit: usize,
) -> Vec<WordEdge> {
    let mut edges = build_word_graph(input, dict, per_code_limit);
    let Some(user_dict) = user_dict else { return edges; };
    let graph = SyllableGraph::build(input, dict);
    for edge in graph.edges {
        for (word, count) in user_dict
            .composition_words(&edge.code)
            .into_iter()
            .take(per_code_limit.max(1))
        {
            if !edges.iter().any(|candidate| {
                candidate.start == edge.start && candidate.end == edge.end && candidate.word == word
            }) {
                edges.push(WordEdge { start: edge.start, end: edge.end, word, weight: count });
            }
        }
    }
    edges
}

pub fn beam_search(
    input: &str,
    dict: &PinyinDict,
    max_sentences: usize,
    beam_width: usize,
    scorer: &impl CandidateScorer,
) -> Vec<RankedSentence> {
    let input = crate::pinyin::normalize_pinyin(input);
    if input.is_empty() || max_sentences == 0 { return Vec::new(); }
    let edges = build_word_graph(&input, dict, beam_width.max(1));
    beam_from_edges(&input, edges, max_sentences, beam_width, scorer)
}

pub fn beam_search_with_user(
    input: &str,
    dict: &PinyinDict,
    user_dict: Option<&UserDict>,
    max_sentences: usize,
    beam_width: usize,
    scorer: &impl CandidateScorer,
) -> Vec<RankedSentence> {
    let input = crate::pinyin::normalize_pinyin(input);
    if input.is_empty() || max_sentences == 0 { return Vec::new(); }
    let edges = build_word_graph_with_user(&input, dict, user_dict, beam_width.max(1));
    beam_from_edges(&input, edges, max_sentences, beam_width, scorer)
}

pub fn beam_search_with_user_context(
    input: &str,
    dict: &PinyinDict,
    user_dict: Option<&UserDict>,
    preceding_word: Option<&str>,
    max_sentences: usize,
    beam_width: usize,
    scorer: &impl CandidateScorer,
) -> Vec<RankedSentence> {
    let input = crate::pinyin::normalize_pinyin(input);
    if input.is_empty() || max_sentences == 0 {
        return Vec::new();
    }
    let edges = build_word_graph_with_user(&input, dict, user_dict, beam_width.max(1));
    beam_from_edges_with_context(&input, edges, preceding_word, max_sentences, beam_width, scorer)
}

fn beam_from_edges(
    input: &str,
    edges: Vec<WordEdge>,
    max_sentences: usize,
    beam_width: usize,
    scorer: &impl CandidateScorer,
) -> Vec<RankedSentence> {
    beam_from_edges_with_context(input, edges, None, max_sentences, beam_width, scorer)
}

fn beam_from_edges_with_context(
    input: &str,
    edges: Vec<WordEdge>,
    preceding_word: Option<&str>,
    max_sentences: usize,
    beam_width: usize,
    scorer: &impl CandidateScorer,
) -> Vec<RankedSentence> {
    let mut states: Vec<Vec<RankedSentence>> = vec![Vec::new(); input.len() + 1];
    states[0].push(RankedSentence { words: Vec::new(), score: 0.0 });
    for end in 1..=input.len() {
        let incoming: Vec<_> = edges.iter().filter(|edge| edge.end == end).cloned().collect();
        for edge in incoming {
            let previous = states[edge.start].clone();
            for sentence in previous {
                let previous_word = sentence
                    .words
                    .last()
                    .map(String::as_str)
                    .or(preceding_word);
                let score = sentence.score + scorer.score_word(previous_word, &edge.word, edge.weight, end == input.len());
                let mut words = sentence.words;
                words.push(edge.word.clone());
                states[end].push(RankedSentence { words, score });
            }
        }
        states[end].sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.words.len().cmp(&b.words.len()))
                .then_with(|| a.words.cmp(&b.words))
        });
        states[end].truncate(beam_width.max(max_sentences));
    }
    let mut result = states.pop().unwrap_or_default();
    result.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.words.len().cmp(&b.words.len()))
            .then_with(|| a.words.cmp(&b.words))
    });
    if let Some(best) = result.first().map(|sentence| sentence.score) {
        result.retain(|sentence| sentence.score >= best - SENTENCE_CUTOFF);
    }
    let mut seen = std::collections::HashSet::new();
    result.retain(|sentence| seen.insert(sentence.words.clone()));
    result.truncate(max_sentences);
    result
}

pub fn default_beam_search(input: &str, dict: &PinyinDict, max_sentences: usize) -> Vec<RankedSentence> {
    beam_search(input, dict, max_sentences, max_sentences.saturating_mul(3).max(7), &BasicScorer::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beam_search_returns_complete_paths() {
        let dict = PinyinDict::from_entries(vec![
            ("zhong".into(), "中".into(), 100),
            ("guo".into(), "国".into(), 90),
            ("zhong guo".into(), "中国".into(), 500),
        ]);
        let result = default_beam_search("zhongguo", &dict, 3);
        assert!(result.iter().any(|s| s.words == vec!["中国"]));
        assert!(result.iter().any(|s| s.words == vec!["中", "国"]));
    }

    #[test]
    fn learned_phrase_is_added_as_a_graph_edge() {
        let dict = PinyinDict::from_entries(vec![("zhong".into(), "中".into(), 100)]);
        let mut user = UserDict::new();
        user.learn("zhong", "钟");
        let edges = build_word_graph_with_user("zhong", &dict, Some(&user), 8);
        assert!(edges.iter().any(|edge| edge.word == "钟"));
    }

    #[test]
    fn beam_search_deduplicates_same_sentence_text() {
        let dict = PinyinDict::from_entries(vec![
            ("zhong".into(), "中".into(), 100),
            ("zhong".into(), "中".into(), 90),
        ]);
        let result = default_beam_search("zhong", &dict, 8);
        assert_eq!(result.iter().filter(|sentence| sentence.words == vec!["中"]).count(), 1);
    }

    #[test]
    fn context_word_is_used_for_first_sentence_score() {
        let dict = PinyinDict::from_entries(vec![
            ("a".into(), "甲".into(), 100),
            ("a".into(), "乙".into(), 90),
        ]);
        let mut scorer = crate::context_score::BigramScorer::new(BasicScorer::default());
        scorer.insert("前", "乙", 10.0);
        let result = beam_search_with_user_context("a", &dict, None, Some("前"), 2, 4, &scorer);
        assert_eq!(result[0].words, vec!["乙"]);
    }
}

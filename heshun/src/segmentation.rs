//! 拼音输入分段图。
//!
//! 参考 librime 的 SyllableGraph，但保持 Heshun 的轻量 Rust 数据结构。
//! 图中的位置是归一化后的 ASCII 拼音字符位置；delimiter 只用于阻止
//! 跨边界连接，不会被当成拼音编码的一部分。

use crate::pinyin::{normalize_pinyin, PinyinDict};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpellingType {
    Normal,
    Abbreviation,
    Completion,
    Correction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeProperties {
    pub spelling_type: SpellingType,
    pub credibility: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyllableEdge {
    pub start: usize,
    pub end: usize,
    pub code: String,
    pub properties: EdgeProperties,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyllableGraph {
    pub input: String,
    pub input_length: usize,
    pub interpreted_length: usize,
    pub edges: Vec<SyllableEdge>,
}

impl SyllableGraph {
    pub fn build(input: &str, dict: &PinyinDict) -> Self {
        let normalized = normalize_pinyin(input);
        let n = normalized.len();
        let mut delimiter_boundaries = Vec::new();
        let mut normalized_pos = 0;
        let mut previous_was_delimiter = false;
        for ch in input.chars() {
            if ch == ' ' || ch == '\'' {
                previous_was_delimiter = normalized_pos > 0;
            } else {
                if previous_was_delimiter {
                    delimiter_boundaries.push(normalized_pos);
                }
                previous_was_delimiter = false;
                normalized_pos += ch.len_utf8();
            }
        }
        let mut edges = Vec::new();
        let mut farthest = 0;

        for start in 0..n {
            let suffix = &normalized[start..];
            for (end, candidate) in dict.matches_prefix(suffix) {
                let end = start + end;
                if end <= n && !delimiter_boundaries.iter().any(|&boundary| start < boundary && boundary < end) {
                    farthest = farthest.max(end);
                    edges.push(SyllableEdge {
                        start,
                        end,
                        code: normalized[start..end].to_string(),
                        properties: EdgeProperties {
                            spelling_type: SpellingType::Normal,
                            credibility: 0,
                        },
                    });
                    // matches_prefix returns one item per dictionary entry;
                    // retain each edge only once per spelling code here.
                    let _ = candidate;
                }
            }
        }

        edges.sort_by_key(|edge| (edge.start, edge.end, edge.code.clone()));
        edges.dedup_by(|a, b| a.start == b.start && a.end == b.end && a.code == b.code);
        Self {
            input: normalized,
            input_length: n,
            interpreted_length: farthest,
            edges,
        }
    }

    pub fn edges_from(&self, start: usize) -> impl Iterator<Item = &SyllableEdge> {
        self.edges.iter().filter(move |edge| edge.start == start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> PinyinDict {
        PinyinDict::from_entries(vec![
            ("zhong".into(), "中".into(), 100),
            ("guo".into(), "国".into(), 90),
            ("zhong guo".into(), "中国".into(), 95),
        ])
    }

    #[test]
    fn graph_contains_short_and_long_paths() {
        let graph = SyllableGraph::build("zhongguo", &dict());
        assert!(graph.edges.iter().any(|e| e.code == "zhong" && e.end == 5));
        assert!(graph.edges.iter().any(|e| e.code == "zhongguo" && e.end == 8));
        assert_eq!(graph.interpreted_length, 8);
    }

    #[test]
    fn graph_normalizes_delimiters_without_crossing_invalid_bytes() {
        let graph = SyllableGraph::build("zhong'guo", &dict());
        assert_eq!(graph.input, "zhongguo");
        assert!(graph.edges.iter().any(|e| e.code == "zhong"));
        assert!(!graph.edges.iter().any(|e| e.code == "zhongguo"));
    }
}

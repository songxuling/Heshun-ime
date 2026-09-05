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
    pub original_start: usize,
    pub original_end: usize,
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
    pub original_input: String,
    pub input: String,
    pub input_length: usize,
    pub interpreted_length: usize,
    pub edges: Vec<SyllableEdge>,
}

impl SyllableGraph {
    pub fn build(input: &str, dict: &PinyinDict) -> Self {
        Self::build_with_options(input, dict, false)
    }

    pub fn build_with_options(input: &str, dict: &PinyinDict, enable_abbreviation: bool) -> Self {
        let normalized = normalize_pinyin(input);
        let n = normalized.len();
        let mut original_ranges = Vec::new();
        let mut delimiter_boundaries = Vec::new();
        let mut normalized_pos = 0usize;
        let mut previous_was_delimiter = false;
        for (original_pos, ch) in input.char_indices() {
            if ch == ' ' || ch == '\'' {
                previous_was_delimiter = normalized_pos > 0;
            } else {
                if previous_was_delimiter {
                    delimiter_boundaries.push(normalized_pos);
                }
                previous_was_delimiter = false;
                normalized_pos += ch.len_utf8();
                original_ranges.push((original_pos, original_pos + ch.len_utf8()));
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
                            original_start: original_ranges
                                .get(start)
                                .map(|range| range.0)
                                .unwrap_or(0),
                            original_end: original_ranges
                                .get(end.saturating_sub(1))
                                .map(|range| range.1)
                                .unwrap_or(0),
                        },
                    });
                    // matches_prefix returns one item per dictionary entry;
                    // retain each edge only once per spelling code here.
                    let _ = candidate;
                }
            }
        }

        if enable_abbreviation {
            for (code, _) in dict.abbreviation_with_codes(&normalized, 64) {
                edges.push(SyllableEdge {
                    start: 0,
                    end: n,
                    code,
                    properties: EdgeProperties {
                        spelling_type: SpellingType::Abbreviation,
                        credibility: -230,
                        original_start: original_ranges.first().map(|range| range.0).unwrap_or(0),
                        original_end: original_ranges.last().map(|range| range.1).unwrap_or(0),
                    },
                });
            }
            for (code, _) in dict.prefix_with_codes(&normalized) {
                if code != normalized {
                    edges.push(SyllableEdge {
                        start: 0,
                        end: n,
                        code,
                        properties: EdgeProperties {
                            spelling_type: SpellingType::Completion,
                            credibility: -300,
                            original_start: original_ranges.first().map(|range| range.0).unwrap_or(0),
                            original_end: original_ranges.last().map(|range| range.1).unwrap_or(0),
                        },
                    });
                }
            }
        }

        // Keep only edges reachable from the beginning, matching Rime's
        // stale-vertex cleanup and preventing suffix matches after junk input
        // from inflating interpreted_length.
        let mut reachable = vec![false; n + 1];
        reachable[0] = true;
        for start in 0..=n {
            if reachable[start] {
                for edge in edges.iter().filter(|edge| edge.start == start) {
                    reachable[edge.end] = true;
                }
            }
        }
        edges.retain(|edge| reachable[edge.start]);
        farthest = (0..=n).rfind(|&position| reachable[position]).unwrap_or(0);

        // Match librime's Syllabifier completion pass: if the reachable graph
        // stops at an unfinished final syllable, consume that tail with a
        // completion edge and query the corresponding complete syllable code.
        if farthest < n {
            let tail = &normalized[farthest..];
            for code in dict.complete_syllables(tail) {
                edges.push(SyllableEdge {
                    start: farthest,
                    end: n,
                    code,
                    properties: EdgeProperties {
                        spelling_type: SpellingType::Completion,
                        credibility: -300,
                        original_start: original_ranges.get(farthest).map(|r| r.0).unwrap_or(0),
                        original_end: original_ranges.last().map(|r| r.1).unwrap_or(0),
                    },
                });
            }
            if edges.iter().any(|edge| {
                edge.start == farthest
                    && edge.end == n
                    && edge.properties.spelling_type == SpellingType::Completion
            }) {
                farthest = n;
            }
        }
        edges.sort_by_key(|edge| (edge.start, edge.end, edge.code.clone()));
        edges.dedup_by(|a, b| a.start == b.start && a.end == b.end && a.code == b.code);
        Self {
            original_input: input.to_string(),
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

    #[test]
    fn graph_records_original_ranges_and_only_reachable_paths() {
        let graph = SyllableGraph::build("zhong'guo", &dict());
        let zhong = graph.edges.iter().find(|e| e.code == "zhong").unwrap();
        assert_eq!(graph.original_input, "zhong'guo");
        assert_eq!(zhong.properties.original_start, 0);
        assert_eq!(zhong.properties.original_end, 5);

        let unreachable = SyllableGraph::build("xzhongguo", &dict());
        assert_eq!(unreachable.interpreted_length, 0);
    }

    #[test]
    fn graph_can_add_abbreviation_edges() {
        let graph = SyllableGraph::build_with_options("zg", &dict(), true);
        assert!(graph.edges.iter().any(|edge| {
            edge.code == "zhongguo" && edge.properties.spelling_type == SpellingType::Abbreviation
        }));
    }

    #[test]
    fn graph_can_add_completion_edges() {
        let graph = SyllableGraph::build("zhon", &dict());
        assert!(graph.edges.iter().any(|edge| {
            edge.code == "zhong" && edge.properties.spelling_type == SpellingType::Completion
        }));
    }

    #[test]
    fn graph_completes_tail_after_multiple_syllables() {
        let dict = PinyinDict::from_entries(vec![
            ("wo".into(), "我".into(), 100),
            ("ai".into(), "爱".into(), 100),
            ("ni".into(), "你".into(), 100),
            ("zhong".into(), "中".into(), 100),
        ]);
        let graph = SyllableGraph::build("woainizhon", &dict);
        assert_eq!(graph.interpreted_length, "woainizhon".len());
        assert!(graph.edges.iter().any(|edge| {
            edge.start == 6 && edge.end == 10 && edge.code == "zhong"
                && edge.properties.spelling_type == SpellingType::Completion
        }));
    }
}

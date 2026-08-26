//! 拼音组句（动态规划），供 script_translator 使用。
//!
//! 给定连续拼音串（如 "zhongguo"）和音码字典，找到所有可行的分词方案，
//! 按词频总分排序返回最佳 N 个句子候选。

use crate::pinyin::{normalize_pinyin, PinyinCandidate, PinyinDict};
use serde::{Deserialize, Serialize};

const MAX_COMPOSE_INPUT_LEN: usize = 64;

/// 一个句子候选：词序列 + 总词频分。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentenceCandidate {
    pub words: Vec<String>,
    pub score: u32,
}

/// 组句器。对输入拼音串做 DP 分词，返回最优候选（按 score 降序）。
///
/// 算法：dp[i] = 覆盖 input[0..i] 的最佳候选列表。
/// 对于每个位置 i，枚举所有 j < i，找到字典中 code == input[j..i] 的词条，
/// 与 dp[j] 组合形成新候选。保留 top `max_results` 个按 score 降序的候选。
pub fn compose(input: &str, dict: &PinyinDict, max_results: usize) -> Vec<SentenceCandidate> {
    let input = normalize_pinyin(input);
    let n = input.len();
    if n == 0 {
        return Vec::new();
    }
    // Keep arbitrary full-Pinyin input editable, but do not let the
    // quadratic DP state grow without bound inside a host key callback.
    if n > MAX_COMPOSE_INPUT_LEN {
        return Vec::new();
    }

    // dp[i] = 覆盖前 i 个字符的最佳候选列表
    let mut dp: Vec<Vec<SentenceCandidate>> = vec![Vec::new(); n + 1];
    dp[0] = vec![SentenceCandidate {
        words: Vec::new(),
        score: 0,
    }];

    let max_results = if max_results == 0 { 64 } else { max_results.max(1) };

    for i in 1..=n {
        // 先克隆 dp[j] 的数据（j < i），避免同时借用
        let prev_data: Vec<Vec<SentenceCandidate>> = dp[..i].to_vec();

        for j in 0..i {
            let segment = &input[j..i];
            // The DP output is capped at max_results. Pinyin entries sharing
            // one code are already ordered by descending frequency, so
            // materialize only the candidates that can affect the top-N result.
            let cands: Vec<PinyinCandidate> = dict.exact_limited(segment, max_results);
            if cands.is_empty() || prev_data[j].is_empty() {
                continue;
            }
            for prev in &prev_data[j] {
                for cand in &cands {
                    let mut words = prev.words.clone();
                    words.push(cand.word.clone());
                    let new_cand = SentenceCandidate {
                        words,
                        score: prev.score + cand.weight,
                    };
                    insert_sorted(&mut dp[i], new_cand, max_results);
                }
            }
        }
    }

    dp[n].clone()
}

/// 保持 vec 按 score 降序，最多保留 limit 个。
fn insert_sorted(vec: &mut Vec<SentenceCandidate>, item: SentenceCandidate, limit: usize) {
    let pos = vec.partition_point(|x| x.score > item.score);
    if vec.len() < limit {
        vec.insert(pos, item);
    } else if pos < vec.len() {
        vec.insert(pos, item);
        vec.truncate(limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pinyin::PinyinDict;

    fn sample_dict() -> PinyinDict {
        PinyinDict::from_entries(vec![
            ("zhong".into(), "中".into(), 100),
            ("zhong".into(), "钟".into(), 80),
            ("zhong".into(), "忠".into(), 60),
            ("zhong guo".into(), "中国".into(), 95),
            ("zhong guo ren".into(), "中国人".into(), 90),
            ("guo".into(), "国".into(), 90),
            ("guo".into(), "过".into(), 70),
            ("guo jia".into(), "国家".into(), 85),
            ("ren".into(), "人".into(), 100),
            ("ren".into(), "仁".into(), 50),
            ("wo".into(), "我".into(), 100),
        ])
    }

    #[test]
    fn compose_single_syllable() {
        let d = sample_dict();
        let result = compose("wo", &d, 5);
        assert!(!result.is_empty());
        assert_eq!(result[0].words, vec!["我"]);
    }

    #[test]
    fn compose_multi_syllable() {
        let d = sample_dict();
        // "zhongguo" → 中国 (95) vs 中 (100) + 国 (90) =190
        let result = compose("zhongguo", &d, 9);
        assert!(!result.is_empty());
        // 最佳应是 中+国 (190) > 中国 (95)
        assert_eq!(result[0].words, vec!["中", "国"]);
        assert!(result[0].score >= 190);

        // 应包含 中国 作为候选（可能需要更多 max_results）
        let has_china = result.iter().any(|c| c.words == vec!["中国"]);
        assert!(has_china, "应包含 '中国' 候选");
    }

    #[test]
    fn compose_three_syllables() {
        let d = sample_dict();
        // "zhongguoren" → 中+国+人 (290) 为最优；中国人(90) 是单个词条，
        // 纯词频模型下分数低于单字组合。这是已知简化（Phase 3 引入语言模型后可优化）。
        let result = compose("zhongguoren", &d, 20);
        assert!(!result.is_empty());
        assert_eq!(result[0].words, vec!["中", "国", "人"]);
        // 多音节词 "中国人" 作为词条也应被检索到（分数低，排在后面）
        assert!(result.iter().any(|c| c.words == vec!["中国人"]));
    }

    #[test]
    fn compose_empty() {
        let d = sample_dict();
        assert!(compose("", &d, 5).is_empty());
        assert!(compose("xx", &d, 5).is_empty());
    }

    #[test]
    fn compose_long_input_is_bounded() {
        let d = sample_dict();
        let input = "a".repeat(65);
        assert!(compose(&input, &d, 9).is_empty());
    }
}
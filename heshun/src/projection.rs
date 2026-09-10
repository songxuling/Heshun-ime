//! 输入拼写投影：把用户原始输入统一投影为词典查询串和显示预编辑。
//!
//! 全拼是 Identity 投影；双拼通过编译期内嵌的 Rime algebra 反向映射投影。
//! 后续的 syllable graph、word graph 和 translation 不区分输入布局。

use crate::pinyin::{normalize_pinyin, PinyinDict};
use crate::zrm::ZrmMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedSpelling {
    pub input: String,
    /// One raw UTF-8 range for each byte in `input`. Pinyin spellings are
    /// ASCII, so graph byte offsets can map directly through this table.
    pub original_ranges: Vec<(usize, usize)>,
    pub delimiter_boundaries: Vec<usize>,
}

#[derive(Clone, Copy)]
pub enum SpellingProjection<'a> {
    FullPinyin,
    DoublePinyin(&'a ZrmMap),
}

impl<'a> SpellingProjection<'a> {
    pub fn from_dict(dict: &'a PinyinDict) -> Self {
        match dict.zrm() {
            Some(map) => Self::DoublePinyin(map),
            None => Self::FullPinyin,
        }
    }

    /// Complete spelling used for dictionary lookup and graph construction.
    pub fn lookup(&self, raw: &str) -> String {
        self.project(raw).input
    }

    /// Project raw input into the common spelling graph coordinate space.
    /// Unlike the legacy whole-string conversion, this retains the raw range
    /// that generated every projected spelling byte.
    pub fn project(&self, raw: &str) -> ProjectedSpelling {
        match self {
            Self::FullPinyin => project_full_pinyin(raw),
            Self::DoublePinyin(map) => project_double_pinyin(raw, map),
        }
    }

    /// Display spelling, retaining an unfinished double-pinyin key.
    pub fn display(&self, raw: &str) -> String {
        match self {
            Self::FullPinyin => normalize_pinyin(raw),
            Self::DoublePinyin(map) => map.to_pinyin_display(raw),
        }
    }

    pub fn is_double_pinyin(&self) -> bool {
        matches!(self, Self::DoublePinyin(_))
    }

    pub fn accepts_key_char(&self, ch: char) -> bool {
        match self {
            Self::FullPinyin => false,
            Self::DoublePinyin(map) => map.accepts_key_char(ch),
        }
    }
}

fn project_full_pinyin(raw: &str) -> ProjectedSpelling {
    let mut input = String::new();
    let mut original_ranges = Vec::new();
    let mut delimiter_boundaries = Vec::new();
    let mut after_delimiter = false;
    for (start, ch) in raw.char_indices() {
        let end = start + ch.len_utf8();
        if matches!(ch, ' ' | '\'') {
            after_delimiter = !input.is_empty();
            continue;
        }
        if after_delimiter {
            delimiter_boundaries.push(input.len());
            after_delimiter = false;
        }
        input.push(ch);
        original_ranges.extend(std::iter::repeat((start, end)).take(ch.len_utf8()));
    }
    ProjectedSpelling { input, original_ranges, delimiter_boundaries }
}

fn project_double_pinyin(raw: &str, map: &ZrmMap) -> ProjectedSpelling {
    let mut input = String::new();
    let mut original_ranges = Vec::new();
    let mut delimiter_boundaries = Vec::new();
    let mut pending: Option<(char, usize, usize)> = None;
    let mut after_delimiter = false;

    for (start, ch) in raw.char_indices() {
        let end = start + ch.len_utf8();
        if matches!(ch, ' ' | '\'') {
            // A lone leading key remains display-only until a second key can
            // form a double-pinyin syllable.
            pending = None;
            after_delimiter = !input.is_empty();
            continue;
        }
        if after_delimiter {
            delimiter_boundaries.push(input.len());
            after_delimiter = false;
        }
        if let Some((first, pair_start, _)) = pending.take() {
            let key: String = [first, ch].iter().collect();
            if let Some(syllable) = map.lookup(&key) {
                append_projected(&mut input, &mut original_ranges, syllable, (pair_start, end));
            } else {
                append_projected(&mut input, &mut original_ranges, &key, (pair_start, end));
            }
        } else {
            pending = Some((ch, start, end));
        }
    }
    ProjectedSpelling { input, original_ranges, delimiter_boundaries }
}

fn append_projected(
    input: &mut String,
    original_ranges: &mut Vec<(usize, usize)>,
    spelling: &str,
    range: (usize, usize),
) {
    input.push_str(spelling);
    original_ranges.extend(std::iter::repeat(range).take(spelling.len()));
}

/// Format a display spelling using the dictionary syllabary and map the raw
/// caret to a display caret. The raw buffer remains untouched by this helper.
pub fn format_preedit(dict: &PinyinDict, raw: &str, cursor: usize) -> (String, usize) {
    let projection = SpellingProjection::from_dict(dict);
    let display_input = projection.display(raw);
    let raw_prefix: String = raw.chars().take(cursor).collect();
    let display_prefix = projection.display(&raw_prefix);
    let raw_prefix_len = display_prefix.chars().count();
    let chars: Vec<char> = display_input.chars().collect();
    let mut output = String::new();
    let mut position = 0usize;
    let mut display_cursor = 0usize;

    while position < chars.len() {
        let mut best_end = None;
        for end in (position + 1..=chars.len()).rev() {
            let syllable: String = chars[position..end].iter().collect();
            if dict.has_syllable(&syllable) {
                best_end = Some(end);
                break;
            }
        }
        let end = best_end.unwrap_or(chars.len());
        if !output.is_empty() {
            output.push('\'');
        }
        if position < raw_prefix_len {
            display_cursor = output.chars().count();
        }
        output.extend(chars[position..end].iter());
        if end <= raw_prefix_len {
            display_cursor = output.chars().count();
        }
        position = end;
    }
    if chars.is_empty() {
        display_cursor = 0;
    } else if raw_prefix_len >= chars.len() {
        display_cursor = output.chars().count();
    }
    (output, display_cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> PinyinDict {
        let mut dict = PinyinDict::from_entries(vec![
            ("zhong".into(), "中".into(), 100),
            ("guo".into(), "国".into(), 100),
        ]);
        let alg = crate::algebra::Algebra::natural_code();
        dict = dict.with_zrm(ZrmMap::build(&["zhong".into(), "guo".into()], &alg));
        dict
    }

    #[test]
    fn full_and_double_pinyin_share_projection_api() {
        let full = PinyinDict::from_entries(vec![("zhongguo".into(), "中国".into(), 100)]);
        assert_eq!(SpellingProjection::from_dict(&full).lookup("zhongguo"), "zhongguo");
        assert_eq!(SpellingProjection::from_dict(&dict()).lookup("vsgo"), "zhongguo");
    }

    #[test]
    fn double_pinyin_display_keeps_incomplete_tail() {
        let d = dict();
        assert_eq!(format_preedit(&d, "vsg", 3), ("zhong'g".into(), 7));
    }
}

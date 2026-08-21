//! 标点引擎（punctuator）—— ASCII 标点 → 中文全角标点映射。
//!
//! 对标 Rime 的 `punctuator` processor。在处理器链中位于 speller 之前，
//! 拦截标点按键并上屏全角字符。
//!
//! 默认映射（半角 → 全角）：Rime 的 `default.yaml` 内建标点集。

use crate::engine::FeedResult;
use crate::processor::{ProcessCtx, ProcessOutcome, Processor};

/// 默认标点映射（半角 ASCII → 全角中文）。
const DEFAULT_HALF_TO_FULL: &[(&str, &str)] = &[
    (",", "，"),
    (".", "。"),
    ("!", "！"),
    ("?", "？"),
    (":", "："),
    (";", "；"),
    ("(", "（"),
    (")", "）"),
    ("[", "【"),
    ("]", "】"),
    ("<", "《"),
    (">", "》"),
    ("@", "·"),
    ("#", "＃"),
    ("$", "￥"),
    ("%", "％"),
    ("^", "……"),
    ("&", "＆"),
    ("*", "＊"),
    ("_", "——"),
    ("-", "－"),
    ("+", "＋"),
    ("=", "＝"),
    ("|", "｜"),
    ("\\", "＼"),
    ("/", "、"),
    ("~", "～"),
    ("`", "｀"),
    ("'", "＇"),
    ("\"", "＂"),
];

pub struct Punctuator {
    /// 半角→全角映射。键为半角字符，值为全角字符。
    map: std::collections::HashMap<char, char>,
    /// 是否启用全角模式（默认启用）。
    full_shape: bool,
}

impl Punctuator {
    pub fn new() -> Self {
        let mut map = std::collections::HashMap::new();
        for (half, full) in DEFAULT_HALF_TO_FULL {
            map.insert(half.chars().next().unwrap(), full.chars().next().unwrap());
        }
        Punctuator { map, full_shape: true }
    }

    pub fn with_full_shape(mut self, enabled: bool) -> Self {
        self.full_shape = enabled;
        self
    }
}

impl Default for Punctuator {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for Punctuator {
    fn name(&self) -> &str {
        "punctuator"
    }

    fn process(&self, key: char, _pending: &str, ctx: &mut ProcessCtx) -> Option<ProcessOutcome> {
        if !self.full_shape || ctx.ascii_mode {
            return None; // 半角或西文模式，不转换标点
        }
        if let Some(&full) = self.map.get(&key) {
            // 上屏全角标点
            return Some(ProcessOutcome::Handled(FeedResult::Committed(full.to_string())));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comma_to_full() {
        let p = Punctuator::new();
        let mut ctx = ProcessCtx::default();
        let r = p.process(',', "", &mut ctx);
        assert!(matches!(r, Some(ProcessOutcome::Handled(FeedResult::Committed(ref s))) if s == "，"));
    }

    #[test]
    fn period_to_full() {
        let p = Punctuator::new();
        let mut ctx = ProcessCtx::default();
        let r = p.process('.', "", &mut ctx);
        assert!(matches!(r, Some(ProcessOutcome::Handled(FeedResult::Committed(ref s))) if s == "。"));
    }

    #[test]
    fn ascii_mode_no_convert() {
        let p = Punctuator::new();
        let mut ctx = ProcessCtx { ascii_mode: true, ..Default::default() };
        assert!(p.process(',', "", &mut ctx).is_none());
    }

    #[test]
    fn letter_passthrough() {
        let p = Punctuator::new();
        let mut ctx = ProcessCtx::default();
        assert!(p.process('a', "", &mut ctx).is_none());
    }
}
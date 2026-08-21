//! 反查（reverse lookup）—— 用另一本字典查当前方案。
//!
//! 对标 Rime 的 `reverse_lookup_translator`。郑码方案用 ` 前缀查拼音：
//! 输入 `` `zhong `` 得到 中/钟/忠 等候选（附郑码编码）。
//!
//! 实现：持有一本 PinyinDict（pinyin_simp），进入反查模式后走 Script 查询逻辑。

use crate::pinyin::PinyinDict;

pub struct ReverseLookup {
    dict: PinyinDict,
    prefix: char,
}

impl ReverseLookup {
    pub fn new(dict: PinyinDict, prefix: char) -> Self {
        ReverseLookup { dict, prefix }
    }

    pub fn prefix(&self) -> char {
        self.prefix
    }

    pub fn dict(&self) -> &PinyinDict {
        &self.dict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let d = PinyinDict::from_entries(vec![
            ("zhong".into(), "中".into(), 100),
            ("zhong".into(), "钟".into(), 80),
        ]);
        let rl = ReverseLookup::new(d, '`');
        assert_eq!(rl.prefix(), '`');
        assert_eq!(rl.dict().exact("zhong").len(), 2);
    }
}
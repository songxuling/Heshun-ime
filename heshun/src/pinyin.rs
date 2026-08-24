//! 音码字典（拼音 → 字词列表），供 script_translator（全拼/双拼）使用。
//!
//! 与形码 [`crate::dict::Dict`] 不同：
//! - 形码编码固定 1~4 码（base-27 数值区间），前缀查询 = 两次二分
//! - 拼音是变长音节串，词条可能是多音节词（如「中国」→ "zhong guo"）。
//!   音节间在 Rime dict 里用空格分隔（`中 国\tzhong guo`），这里统一去掉空格
//!   存成连续串 "zhongguo"，匹配时也去掉空格比较。
//!
//! 每个词条对应一个候选，按词频（weight）降序排列。
//!
//! 二进制格式 "ZPY1"（小端）:
//! ```text
//! u32 magic       = 0x3159505A ("ZPY1")
//! u32 version     = 1
//! u32 entry_count
//! u32 blob_len
//! entry_count × { u32 code_off, u16 code_len, u32 word_off, u16 word_len, u32 weight }
//! blob_len × u8   // 拼音编码 + 字词的 UTF-8 拼接
//! ```
//! 条目按 code（连续拼音）升序排列，供二分查找。

use crate::zrm::ZrmMap;

const MAGIC: u32 = 0x3159505A;
const VERSION: u32 = 1;

/// 一个拼音候选：字词 + 词频。
#[derive(Debug, Clone)]
pub struct PinyinCandidate {
    pub word: String,
    pub weight: u32,
}

/// 去除拼音中的音节分隔符（空格），得到连续串，用于匹配。
pub fn normalize_pinyin(code: &str) -> String {
    code.chars().filter(|&c| c != ' ' && c != '\'').collect()
}

/// 音码字典。条目按 code（连续拼音）升序排列。
/// 可内嵌双拼反向映射（ZRM1 扩展段），供双拼模式运行时按键→全拼转换。
pub struct PinyinDict {
    codes: Vec<String>,   // 连续拼音（已去空格），升序
    words: Vec<String>,
    weights: Vec<u32>,
    zrm: Option<ZrmMap>,
}

impl PinyinDict {
    /// 从原始条目构建：Vec<(拼音码(可含空格), 字词, 词频)>。
    /// 内部：去掉空格、按拼音码升序排序（同码按词频降序）。
    pub fn from_entries(mut entries: Vec<(String, String, u32)>) -> PinyinDict {
        for (code, _, _) in entries.iter_mut() {
            *code = normalize_pinyin(code);
        }
        // 按 (code, weight 降序) 排序：先 code 升序，同码 weight 降序
        entries.sort_by(|a, b| {
            a.0.cmp(&b.0).then(b.2.cmp(&a.2))
        });
        let mut codes = Vec::with_capacity(entries.len());
        let mut words = Vec::with_capacity(entries.len());
        let mut weights = Vec::with_capacity(entries.len());
        for (code, word, weight) in entries {
            codes.push(code);
            words.push(word);
            weights.push(weight);
        }
        PinyinDict { codes, words, weights, zrm: None }
    }

    /// 附加双拼反向映射。
    pub fn with_zrm(mut self, zrm: ZrmMap) -> Self {
        self.zrm = Some(zrm);
        self
    }

    /// 获取双拼反向映射（若有）。
    pub fn zrm(&self) -> Option<&ZrmMap> {
        self.zrm.as_ref()
    }

    pub fn entry_count(&self) -> usize {
        self.codes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }

    fn word(&self, i: usize) -> &str {
        &self.words[i]
    }

    /// 精确匹配：连续拼音 == code 的所有候选（词频降序）。
    pub fn exact(&self, code: &str) -> Vec<PinyinCandidate> {
        let code = normalize_pinyin(code);
        let lo = self.codes.partition_point(|c| c.as_str() < code.as_str());
        let hi = self.codes.partition_point(|c| c.as_str() <= code.as_str());
        (lo..hi)
            .map(|i| PinyinCandidate { word: self.word(i).to_string(), weight: self.weights[i] })
            .collect()
    }

    /// 前缀匹配：连续拼音以 input 开头的所有候选。
    /// 用于「输入未完整音节」时的候选提示。
    /// 注意：这里返回的是词条的编码前缀匹配，不是音节级前缀。
    pub fn prefix(&self, input: &str) -> Vec<PinyinCandidate> {
        let input = normalize_pinyin(input);
        let lo = self.codes.partition_point(|c| c.as_str() < input.as_str());
        // `starts_with(input)` is not a monotonic predicate, so it cannot be
        // used directly with partition_point. Use the lexicographic interval
        // [input, input + '{') for lowercase ASCII pinyin codes.
        let mut upper = input.clone();
        upper.push('{');
        let hi = self.codes.partition_point(|c| c.as_str() < upper.as_str());
        (lo..hi)
            .map(|i| PinyinCandidate { word: self.word(i).to_string(), weight: self.weights[i] })
            .collect()
    }

    /// 组句用：找出所有「连续拼音 == input 的某段」的词条。
    /// 返回 (结束位置, 候选)。这是音节边界无关的字符串前缀匹配。
    /// 供 script_translator 做 DP 组句时调用。
    pub fn matches_prefix(&self, input: &str) -> Vec<(usize, PinyinCandidate)> {
        let input = normalize_pinyin(input);
        let mut out = Vec::new();
        // 找所有 code 是 input 前缀的词条（code 短于等于 input 且匹配）
        for i in 0..self.codes.len() {
            let c = &self.codes[i];
            if c.len() > input.len() {
                continue;
            }
            if input.starts_with(c.as_str()) {
                out.push((
                    c.len(),
                    PinyinCandidate { word: self.word(i).to_string(), weight: self.weights[i] },
                ));
            }
        }
        // 按词频降序排（保持 DP 选择最优）
        out.sort_by(|a, b| b.1.weight.cmp(&a.1.weight));
        out
    }

    /// 判断某个连续拼音是否在字典中（供分段器判断合法音节/词）。
    pub fn has_code(&self, code: &str) -> bool {
        let code = normalize_pinyin(code);
        self.codes.binary_search(&code).is_ok()
    }

    /// 序列化为二进制格式。
    pub fn save(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        let mut blob = Vec::new();
        let mut code_off = Vec::with_capacity(self.codes.len());
        let mut code_len = Vec::with_capacity(self.codes.len());
        let mut word_off = Vec::with_capacity(self.words.len());
        let mut word_len = Vec::with_capacity(self.words.len());
        for c in &self.codes {
            code_off.push(blob.len() as u32);
            code_len.push(c.len() as u16);
            blob.extend_from_slice(c.as_bytes());
        }
        for word in &self.words {
            word_off.push(blob.len() as u32);
            word_len.push(word.len() as u16);
            blob.extend_from_slice(word.as_bytes());
        }

        w.write_all(&MAGIC.to_le_bytes())?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&(self.codes.len() as u32).to_le_bytes())?;
        w.write_all(&(blob.len() as u32).to_le_bytes())?;
        for i in 0..self.codes.len() {
            w.write_all(&code_off[i].to_le_bytes())?;
            w.write_all(&code_len[i].to_le_bytes())?;
            w.write_all(&word_off[i].to_le_bytes())?;
            w.write_all(&word_len[i].to_le_bytes())?;
            w.write_all(&self.weights[i].to_le_bytes())?;
        }
        w.write_all(&blob)?;

        // ZRM1 扩展段（可选）：双拼反向映射，追加在 ZPY1 主体之后
        if let Some(zrm) = &self.zrm {
            zrm.save(w)?;
        }
        Ok(())
    }

    /// 从二进制格式加载。data 可含 ZRM1 扩展段（双拼映射）。
    pub fn load(data: &[u8]) -> Result<PinyinDict, String> {
        fn rd32(data: &[u8], off: usize) -> Result<u32, String> {
            data.get(off..off + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| "文件截断".into())
        }
        fn rd16(data: &[u8], off: usize) -> Result<u16, String> {
            data.get(off..off + 2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| "文件截断".into())
        }

        if rd32(data, 0)? != MAGIC {
            return Err("魔数不对（不是 ZPY1 码表）".into());
        }
        if rd32(data, 4)? != VERSION {
            return Err("版本不支持".into());
        }
        let n = rd32(data, 8)? as usize;
        let blob_len = rd32(data, 12)? as usize;

        let hdr = 16;
        let ent_end = hdr + n * 16; // u32+u16+u32+u16+u32 = 16
        let blob_end = ent_end + blob_len;
        if data.len() < blob_end {
            return Err("文件截断".into());
        }

        let mut code_off = Vec::with_capacity(n);
        let mut code_len = Vec::with_capacity(n);
        let mut word_off = Vec::with_capacity(n);
        let mut word_len = Vec::with_capacity(n);
        let mut weights = Vec::with_capacity(n);
        for i in 0..n {
            let o = hdr + i * 16;
            code_off.push(rd32(data, o)?);
            code_len.push(rd16(data, o + 4)?);
            word_off.push(rd32(data, o + 6)?);
            word_len.push(rd16(data, o + 10)?);
            weights.push(rd32(data, o + 12)?);
        }

        let blob = &data[ent_end..blob_end];
        let blob_str = std::str::from_utf8(blob).map_err(|_| "blob 不是合法 UTF-8")?;

        let mut codes = Vec::with_capacity(n);
        let mut words = Vec::with_capacity(n);
        for i in 0..n {
            let s = code_off[i] as usize;
            let e = s + code_len[i] as usize;
            codes.push(blob_str[s..e].to_string());
            let s = word_off[i] as usize;
            let e = s + word_len[i] as usize;
            words.push(blob_str[s..e].to_string());
        }

        // 尝试解析 ZRM1 扩展段（双拼映射）
        let mut zrm = None;
        if data.len() > blob_end + 4 {
            let ext = &data[blob_end..];
            if ext.len() >= 4 && u32::from_le_bytes(ext[..4].try_into().unwrap()) == 0x31524D5A {
                zrm = Some(ZrmMap::load(ext)?);
            }
        }

        Ok(PinyinDict { codes, words, weights, zrm })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PinyinDict {
        PinyinDict::from_entries(vec![
            ("zhong".into(), "中".into(), 100),
            ("zhong".into(), "钟".into(), 80),
            ("zhong".into(), "忠".into(), 60),
            ("zhong guo".into(), "中国".into(), 95),
            ("guo".into(), "国".into(), 90),
            ("guo".into(), "过".into(), 70),
            ("wo".into(), "我".into(), 100),
        ])
    }

    #[test]
    fn normalize() {
        assert_eq!(normalize_pinyin("zhong guo"), "zhongguo");
        assert_eq!(normalize_pinyin("zhong'guo"), "zhongguo");
    }

    #[test]
    fn exact_lookup_sorted_by_weight() {
        let d = sample();
        let c = d.exact("zhong");
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].word, "中");
        assert_eq!(c[1].word, "钟");
        assert_eq!(c[0].weight, 100);
        // 多音节词 exact
        let china = d.exact("zhong guo");
        assert_eq!(china.len(), 1);
        assert_eq!(china[0].word, "中国");
    }

    #[test]
    fn exact_missing() {
        let d = sample();
        assert!(d.exact("xx").is_empty());
    }

    #[test]
    fn prefix_lookup() {
        let d = sample();
        let p = d.prefix("zhong");
        assert!(!p.is_empty());
        assert!(p.iter().any(|c| c.word == "中"));
    }

    #[test]
    fn prefix_lookup_stops_at_next_code() {
        let d = PinyinDict::from_entries(vec![
            ("ni".into(), "你".into(), 100),
            ("nihao".into(), "你好".into(), 100),
            ("nian".into(), "年".into(), 100),
            ("nj".into(), "错误边界".into(), 100),
        ]);
        let words: Vec<_> = d.prefix("ni").into_iter().map(|c| c.word).collect();
        assert_eq!(words, vec!["你", "年", "你好"]);
        assert!(!words.iter().any(|w| *w == "错误边界"));
    }

    #[test]
    fn matches_prefix_for_composition() {
        let d = sample();
        // "zhongguo" 应匹配 "zhong"(中/钟/忠) 和 "zhongguo"(中国)
        let m = d.matches_prefix("zhongguo");
        assert!(m.iter().any(|(len, c)| *len == 5 && c.word == "中")); // "zhong"=5
        assert!(m.iter().any(|(len, c)| *len == 8 && c.word == "中国")); // "zhongguo"=8
    }

    #[test]
    fn save_load_roundtrip() {
        let d = sample();
        let mut buf = Vec::new();
        d.save(&mut buf).unwrap();
        let d2 = PinyinDict::load(&buf).unwrap();
        assert_eq!(d2.entry_count(), 7);
        let c = d2.exact("guo");
        assert_eq!(c[0].word, "国");
        assert_eq!(c[0].weight, 90);
        let china = d2.exact("zhong guo");
        assert_eq!(china[0].word, "中国");
    }
}
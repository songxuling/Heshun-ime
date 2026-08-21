//! 码表：编码(1~4码, a-z) → 字词列表
//!
//! 编码用 base-27 大端表示（a=1..z=26, 0=终止符），补齐到 4 位。
//! 这样「同一前缀的所有编码」在数值上构成连续区间 —— 前缀查询 = 两次二分。
//!
//! 二进制格式 "ZMD1"（小端）:
//! ```text
//! u32 magic      = 0x31444D5A ("ZMD1")
//! u32 version    = 1
//! u32 entry_count
//! u32 blob_len
//! entry_count × { u32 code, u32 word_off, u16 word_len }   // 按 code 升序
//! blob_len × u8  // 所有字词的 UTF-8 拼接
//! ```

const MAGIC: u32 = 0x31444D5A;
const VERSION: u32 = 1;
pub const MAX_CODE_LEN: usize = 4;

/// 编码 → base-27 数值；非法（非 a-z、长度 0 或 >4）返回 None。
pub fn encode_code(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > MAX_CODE_LEN {
        return None;
    }
    let mut v: u32 = 0;
    for (i, &c) in b.iter().enumerate() {
        if !c.is_ascii_lowercase() {
            return None;
        }
        v += ((c - b'a') as u32 + 1) * 27u32.pow((MAX_CODE_LEN - 1 - i) as u32);
    }
    Some(v)
}

/// base-27 数值 → 编码字符串（显示用）。
pub fn decode_code(mut v: u32) -> String {
    let mut out = [0u8; MAX_CODE_LEN];
    let mut len = 0;
    for i in (0..MAX_CODE_LEN).rev() {
        let d = v / 27u32.pow(i as u32);
        v %= 27u32.pow(i as u32);
        if d == 0 {
            break;
        }
        out[len] = b'a' + (d as u8 - 1);
        len += 1;
    }
    String::from_utf8_lossy(&out[..len]).into_owned()
}

#[derive(Debug, Clone)]
pub struct Candidate<'a> {
    pub word: &'a str,
    pub code: String,
}

pub struct Dict {
    codes: Vec<u32>,     // 按 code 升序（同码保持码表原序）
    word_off: Vec<u32>,
    word_len: Vec<u16>,
    blob: Vec<u8>,
}

impl Dict {
    /// 从 (编码, 字词) 构建；内部排序。同码多条 = 候选列表，保留原序。
    pub fn from_entries(mut entries: Vec<(u32, String)>) -> Dict {
        entries.sort_by_key(|&(v, _)| v); // 稳定排序：同码保持原序
        let mut blob = Vec::new();
        let mut codes = Vec::with_capacity(entries.len());
        let mut word_off = Vec::with_capacity(entries.len());
        let mut word_len = Vec::with_capacity(entries.len());
        for (v, w) in entries {
            word_off.push(blob.len() as u32);
            word_len.push(w.len() as u16);
            blob.extend_from_slice(w.as_bytes());
            codes.push(v);
        }
        Dict { codes, word_off, word_len, blob }
    }

    pub fn len(&self) -> usize {
        self.codes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }

    fn word(&self, i: usize) -> &str {
        let s = self.word_off[i] as usize;
        let e = s + self.word_len[i] as usize;
        std::str::from_utf8(&self.blob[s..e]).expect("blob 必须是合法 UTF-8")
    }

    /// 精确等于 v 的条目区间。
    pub fn exact_range(&self, v: u32) -> std::ops::Range<usize> {
        let lo = self.codes.partition_point(|&x| x < v);
        let hi = self.codes.partition_point(|&x| x <= v);
        lo..hi
    }

    /// 以 prefix（已编码、长度 prefix_len）开头的所有条目区间。
    pub fn prefix_range(&self, prefix: u32, prefix_len: usize) -> std::ops::Range<usize> {
        let shift = 27u32.pow((MAX_CODE_LEN - prefix_len) as u32);
        let lo_v = prefix; // encode_code 已把前缀放在高位
        let hi_v = prefix + shift;
        let lo = self.codes.partition_point(|&x| x < lo_v);
        let hi = self.codes.partition_point(|&x| x < hi_v);
        lo..hi
    }

    /// 精确匹配的候选（输入恰好是某条目的完整编码）。
    pub fn exact(&self, code: &str) -> Vec<Candidate<'_>> {
        let Some(v) = encode_code(code) else { return Vec::new() };
        self.exact_range(v)
            .map(|i| Candidate { word: self.word(i), code: code.to_string() })
            .collect()
    }

    /// 前缀匹配的候选（编码以 input 开头的全部条目）。
    /// limit=0 表示不限。排序即码表的字典序（同前缀链天然短码在前）。
    pub fn prefix(&self, input: &str, limit: usize) -> Vec<Candidate<'_>> {
        let Some(v) = encode_code(input) else { return Vec::new() };
        let r = self.prefix_range(v, input.len());
        let take = if limit == 0 { r.len() } else { limit.min(r.len()) };
        (r.start..r.start + take)
            .map(|i| Candidate { word: self.word(i), code: decode_code(self.codes[i]) })
            .collect()
    }

    /// 反查：返回某个字词在本形码表中的全部编码，按码表顺序。
    /// 用于「拼音 → 字词 → 形码」的反查候选注释。
    pub fn codes_for_word(&self, word: &str) -> Vec<String> {
        (0..self.codes.len())
            .filter(|&i| self.word(i) == word)
            .map(|i| decode_code(self.codes[i]))
            .collect()
    }

    /// 序列化为二进制格式。
    pub fn save(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        w.write_all(&MAGIC.to_le_bytes())?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&(self.codes.len() as u32).to_le_bytes())?;
        w.write_all(&(self.blob.len() as u32).to_le_bytes())?;
        for i in 0..self.codes.len() {
            w.write_all(&self.codes[i].to_le_bytes())?;
            w.write_all(&self.word_off[i].to_le_bytes())?;
            w.write_all(&self.word_len[i].to_le_bytes())?;
        }
        w.write_all(&self.blob)?;
        Ok(())
    }

    /// 从二进制格式加载。
    pub fn load(data: &[u8]) -> Result<Dict, String> {
        fn rd32(data: &[u8], off: usize) -> Result<u32, String> {
            data.get(off..off + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| "文件截断".into())
        }
        if rd32(data, 0)? != MAGIC {
            return Err("魔数不对（不是 ZMD1 码表）".into());
        }
        if rd32(data, 4)? != VERSION {
            return Err("版本不支持".into());
        }
        let n = rd32(data, 8)? as usize;
        let blob_len = rd32(data, 12)? as usize;
        let hdr = 16;
        let ent_end = hdr + n * 10;
        if data.len() < ent_end + blob_len {
            return Err("文件截断（条目或字词区不完整）".into());
        }
        let mut codes = Vec::with_capacity(n);
        let mut word_off = Vec::with_capacity(n);
        let mut word_len = Vec::with_capacity(n);
        for i in 0..n {
            let o = hdr + i * 10;
            codes.push(u32::from_le_bytes(data[o..o + 4].try_into().unwrap()));
            word_off.push(u32::from_le_bytes(data[o + 4..o + 8].try_into().unwrap()));
            word_len.push(u16::from_le_bytes(data[o + 8..o + 10].try_into().unwrap()));
        }
        let blob = data[ent_end..ent_end + blob_len].to_vec();
        if std::str::from_utf8(&blob).is_err() {
            return Err("字词区不是合法 UTF-8".into());
        }
        Ok(Dict { codes, word_off, word_len, blob })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        for s in ["a", "aa", "jivv", "zzzz", "abcd"] {
            let v = encode_code(s).unwrap();
            assert_eq!(decode_code(v), s, "{s}");
        }
        assert!(encode_code("").is_none());
        assert!(encode_code("abcde").is_none());
        assert!(encode_code("aB").is_none());
        assert!(encode_code("a1").is_none());
    }

    #[test]
    fn prefix_order_and_range() {
        // 前缀区间的数值性质：aa 落在 a 的区间内，b 不在
        let va = encode_code("a").unwrap();
        let vaa = encode_code("aa").unwrap();
        let vb = encode_code("b").unwrap();
        let shift = 27u32.pow(3);
        assert!(vaa >= va && vaa < va + shift);
        assert!(vb >= va + shift);
    }

    #[test]
    fn exact_and_prefix() {
        let d = Dict::from_entries(vec![
            (encode_code("j").unwrap(), "中".into()),
            (encode_code("jivv").unwrap(), "中".into()),
            (encode_code("ji").unwrap(), "虫".into()),
            (encode_code("aa").unwrap(), "一下".into()),
        ]);
        assert_eq!(d.exact("j").len(), 1);
        assert_eq!(d.exact("jivv")[0].word, "中");
        let p = d.prefix("j", 0);
        assert_eq!(p.len(), 3); // j, ji, jivv
        assert_eq!(p[0].code, "j"); // 短码在前（字典序性质）
        assert!(d.exact("zz").is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let d = Dict::from_entries(vec![
            (encode_code("a").unwrap(), "一".into()),
            (encode_code("aa").unwrap(), "一下".into()),
        ]);
        let mut buf = Vec::new();
        d.save(&mut buf).unwrap();
        let d2 = Dict::load(&buf).unwrap();
        assert_eq!(d2.len(), 2);
        assert_eq!(d2.exact("a")[0].word, "一");
        assert_eq!(d2.exact("aa")[0].word, "一下");
    }
}

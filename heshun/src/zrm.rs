//! 双拼反向映射（keystroke → 全拼），供 script_translator 双拼模式使用。
//!
//! 自然码双拼每音节恰好 2 键（如 "vs"→"zhong", "go"→"guo"）。
//! build 阶段对所有拼音音节应用 algebra.transform() 得到双拼键，
//! 构建 HashMap<2键, 全拼音节>。runtime 阶段输入按键后，
//! 把 2 键映射回全拼，再交给 DP 组句。
//!
//! 二进制格式 "ZRM1"（小端），作为 ZPY1 的扩展段：
//! ```text
//! u32 magic       = 0x31524D5A ("ZRM1")
//! u32 version     = 1
//! u32 entry_count
//! u32 blob_len
//! entry_count × { u32 key_off, u16 key_len, u32 py_off, u16 py_len }
//! blob_len × u8   // 键 + 拼音的 UTF-8 拼接
//! ```
//! 键（2字符）升序排列，供二分查找。

const MAGIC: u32 = 0x31524D5A;
const VERSION: u32 = 1;

/// 双拼反向映射表。键升序。
pub struct ZrmMap {
    keys: Vec<String>,      // 双拼键（2字符），升序
    pinyins: Vec<String>,   // 对应全拼音节
}

impl ZrmMap {
    /// 从全拼音节集合构建：应用 algebra 得到双拼键，建反向映射。
    ///
    /// syllables: 词典中出现的全部不同音节（如 "zhong", "guo", "zhong guo" 拆开的单音节）。
    /// 注意：只对单音节做映射；多音节词（含空格）应先用空格拆开。
    pub fn build(syllables: &[String], algebra: &crate::algebra::Algebra) -> ZrmMap {
        // 收集去重的单音节
        let mut unique: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for s in syllables {
            for part in s.split(|c| c == ' ' || c == '\'') {
                if !part.is_empty() {
                    unique.insert(part.to_string());
                }
            }
        }

        let mut entries: Vec<(String, String)> = Vec::new();
        for syll in unique {
            let key = algebra.transform(&syll);
            // 双拼键必须是 2 字符；否则跳过（如被 erase 规则删除）
            if key.chars().count() == 2 {
                entries.push((key, syll));
            }
        }
        // 按键升序（同键保留首个，罕见冲突）
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        entries.dedup_by(|a, b| a.0 == b.0);

        let mut keys = Vec::with_capacity(entries.len());
        let mut pinyins = Vec::with_capacity(entries.len());
        for (k, p) in entries {
            keys.push(k);
            pinyins.push(p);
        }
        ZrmMap { keys, pinyins }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// 单键 → 全拼音节。找不到返回 None。
    pub fn lookup(&self, key: &str) -> Option<&str> {
        match self.keys.binary_search(&key.to_string()) {
            Ok(i) => Some(&self.pinyins[i]),
            Err(_) => None,
        }
    }

    /// 按键序列 → 全拼串（每 2 键映射一个音节，直接拼接）。
    /// 奇数长度时，末尾 1 键无法映射，忽略之（该键是下一音节的起始）。
    pub fn to_pinyin(&self, keystrokes: &str) -> String {
        let chars: Vec<char> = keystrokes.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i + 1 < chars.len() {
            let key: String = chars[i..i + 2].iter().collect();
            match self.lookup(&key) {
                Some(py) => out.push_str(py),
                None => {
                    // 非法键：原样保留（保持与 pending 显示一致）
                    out.push_str(&key);
                }
            }
            i += 2;
        }
        out
    }

    /// 序列化。
    pub fn save(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        let mut blob = Vec::new();
        let mut key_off = Vec::with_capacity(self.keys.len());
        let mut key_len = Vec::with_capacity(self.keys.len());
        let mut py_off = Vec::with_capacity(self.pinyins.len());
        let mut py_len = Vec::with_capacity(self.pinyins.len());
        for k in &self.keys {
            key_off.push(blob.len() as u32);
            key_len.push(k.len() as u16);
            blob.extend_from_slice(k.as_bytes());
        }
        for p in &self.pinyins {
            py_off.push(blob.len() as u32);
            py_len.push(p.len() as u16);
            blob.extend_from_slice(p.as_bytes());
        }

        w.write_all(&MAGIC.to_le_bytes())?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&(self.keys.len() as u32).to_le_bytes())?;
        w.write_all(&(blob.len() as u32).to_le_bytes())?;
        for i in 0..self.keys.len() {
            w.write_all(&key_off[i].to_le_bytes())?;
            w.write_all(&key_len[i].to_le_bytes())?;
            w.write_all(&py_off[i].to_le_bytes())?;
            w.write_all(&py_len[i].to_le_bytes())?;
        }
        w.write_all(&blob)?;
        Ok(())
    }

    /// 反序列化。data 应为整个 ZRM1 段（从 magic 开始）。
    pub fn load(data: &[u8]) -> Result<ZrmMap, String> {
        fn rd32(data: &[u8], off: usize) -> Result<u32, String> {
            data.get(off..off + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| "ZRM1 文件截断".into())
        }
        fn rd16(data: &[u8], off: usize) -> Result<u16, String> {
            data.get(off..off + 2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| "ZRM1 文件截断".into())
        }

        if rd32(data, 0)? != MAGIC {
            return Err("魔数不对（不是 ZRM1 映射表）".into());
        }
        if rd32(data, 4)? != VERSION {
            return Err("版本不支持".into());
        }
        let n = rd32(data, 8)? as usize;
        let blob_len = rd32(data, 12)? as usize;

        let hdr = 16;
        let ent_end = hdr + n * 12; // u32+u16+u32+u16 = 12
        if data.len() < ent_end + blob_len {
            return Err("ZRM1 文件截断".into());
        }

        let mut key_off = Vec::with_capacity(n);
        let mut key_len = Vec::with_capacity(n);
        let mut py_off = Vec::with_capacity(n);
        let mut py_len = Vec::with_capacity(n);
        for i in 0..n {
            let o = hdr + i * 12;
            key_off.push(rd32(data, o)?);
            key_len.push(rd16(data, o + 4)?);
            py_off.push(rd32(data, o + 6)?);
            py_len.push(rd16(data, o + 10)?);
        }

        let blob = &data[ent_end..ent_end + blob_len];
        let blob_str = std::str::from_utf8(blob).map_err(|_| "blob 不是合法 UTF-8")?;

        let mut keys = Vec::with_capacity(n);
        let mut pinyins = Vec::with_capacity(n);
        for i in 0..n {
            let s = key_off[i] as usize;
            let e = s + key_len[i] as usize;
            keys.push(blob_str[s..e].to_string());
            let s = py_off[i] as usize;
            let e = s + py_len[i] as usize;
            pinyins.push(blob_str[s..e].to_string());
        }

        Ok(ZrmMap { keys, pinyins })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::Algebra;

    #[test]
    fn build_and_lookup() {
        let alg = Algebra::natural_code();
        let sylls = ["zhong".to_string(), "guo".to_string(), "shu".to_string(), "wo".to_string()];
        let map = ZrmMap::build(&sylls, &alg);
        assert!(map.len() >= 4);
        assert_eq!(map.lookup("vs"), Some("zhong"));
        assert_eq!(map.lookup("go"), Some("guo"));
        assert_eq!(map.lookup("uu"), Some("shu"));
        assert_eq!(map.lookup("wo"), Some("wo"));
    }

    #[test]
    fn to_pinyin_sequence() {
        let alg = Algebra::natural_code();
        let sylls = ["zhong".to_string(), "guo".to_string()];
        let map = ZrmMap::build(&sylls, &alg);
        // "vsgo" → zhong guo
        assert_eq!(map.to_pinyin("vsgo"), "zhongguo");
        // 奇数长度：末尾键忽略
        assert_eq!(map.to_pinyin("vs"), "zhong");
    }

    #[test]
    fn save_load_roundtrip() {
        let alg = Algebra::natural_code();
        let sylls = ["zhong".to_string(), "guo".to_string(), "shu".to_string()];
        let map = ZrmMap::build(&sylls, &alg);
        let mut buf = Vec::new();
        map.save(&mut buf).unwrap();
        let map2 = ZrmMap::load(&buf).unwrap();
        assert_eq!(map2.len(), map.len());
        assert_eq!(map2.lookup("vs"), Some("zhong"));
        assert_eq!(map2.lookup("go"), Some("guo"));
    }

    #[test]
    fn zero_initial_syllables() {
        let alg = Algebra::natural_code();
        // 零声母：a→aa, o→oo, e→ee, ang→ah, er→er
        let sylls = ["a".to_string(), "o".to_string(), "e".to_string(), "ang".to_string(), "er".to_string()];
        let map = ZrmMap::build(&sylls, &alg);
        assert_eq!(map.lookup("aa"), Some("a"));
        assert_eq!(map.lookup("oo"), Some("o"));
        assert_eq!(map.lookup("ee"), Some("e"));
        assert_eq!(map.lookup("ah"), Some("ang"));
        assert_eq!(map.lookup("er"), Some("er"));
    }
}
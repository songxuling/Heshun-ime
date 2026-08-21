//! 用户词典（user dictionary）—— 记录用户选词，提升常用词优先级。
//!
//! 对标 Rime 的 `*.userdb`。轻量实现：内存 HashMap + 可选 JSON 持久化。
//! - 记录：用户每次选词 (编码, 字词) 的累计次数
//! - 应用：候选排序时，被选过的词排在前面（boost）
//!
//! 持久化格式：JSON（`serde`），`{ "编码": { "字词": 次数, ... }, ... }`

use std::collections::HashMap;

use crate::engine::Candidate;

#[derive(Debug, Default)]
pub struct UserDict {
    /// code(编码，形码=码，音码=拼音) → (字词 → 累计选择次数)
    counts: HashMap<String, HashMap<String, u32>>,
}

impl UserDict {
    pub fn new() -> Self {
        UserDict { counts: HashMap::new() }
    }

    /// 记录一次选词。
    pub fn learn(&mut self, code: &str, word: &str) {
        *self.counts.entry(code.to_string()).or_default().entry(word.to_string()).or_insert(0) += 1;
    }

    /// 查询某编码下某字词的选中次数。
    pub fn count(&self, code: &str, word: &str) -> u32 {
        self.counts.get(code).and_then(|m| m.get(word)).copied().unwrap_or(0)
    }

    /// 某编码下是否有用户记录。
    pub fn has_code(&self, code: &str) -> bool {
        self.counts.contains_key(code)
    }

    /// 某编码下的用户选词，按次数降序返回 (字词, 次数)。
    pub fn words_for(&self, code: &str) -> Vec<(String, u32)> {
        let Some(m) = self.counts.get(code) else { return Vec::new() };
        let mut v: Vec<(String, u32)> = m.iter().map(|(w, c)| (w.clone(), *c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    }

    /// 持久化到 JSON 文件。
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.counts)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// 从 JSON 文件加载。
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let counts: HashMap<String, HashMap<String, u32>> =
            serde_json::from_str(&text)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(UserDict { counts })
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// 按用户选择次数稳定提升候选；没有学习记录的候选保持原顺序。
    pub fn reorder_candidates(&self, code: &str, candidates: &mut Vec<Candidate>) {
        candidates.sort_by(|a, b| {
            self.count(code, &b.word)
                .cmp(&self.count(code, &a.word))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_and_count() {
        let mut ud = UserDict::new();
        ud.learn("jivv", "中");
        ud.learn("jivv", "中");
        ud.learn("jivv", "虽");
        assert_eq!(ud.count("jivv", "中"), 2);
        assert_eq!(ud.count("jivv", "虽"), 1);
        assert_eq!(ud.count("zzzz", "断"), 0);
    }

    #[test]
    fn words_for_sorted() {
        let mut ud = UserDict::new();
        ud.learn("zhong", "中");
        ud.learn("zhong", "钟");
        ud.learn("zhong", "钟");
        let w = ud.words_for("zhong");
        assert_eq!(w[0], ("钟".to_string(), 2));
        assert_eq!(w[1], ("中".to_string(), 1));
    }

    #[test]
    fn reorder_candidates_promotes_learned_word() {
        let mut ud = UserDict::new();
        ud.learn("zhong", "钟");
        ud.learn("zhong", "钟");
        let mut candidates = vec![
            Candidate { word: "中".into(), code: "zhong".into() },
            Candidate { word: "钟".into(), code: "zhong".into() },
        ];
        ud.reorder_candidates("zhong", &mut candidates);
        assert_eq!(candidates[0].word, "钟");
    }

    #[test]
    fn save_creates_parent_directory() {
        let mut ud = UserDict::new();
        ud.learn("aa", "一下");
        let dir = std::env::temp_dir().join(format!("hs_userdict_dir_{}", std::process::id()));
        let path = dir.join("nested").join("dict.json");
        ud.save(&path).unwrap();
        assert_eq!(UserDict::load(&path).unwrap().count("aa", "一下"), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn save_load_roundtrip() {
        let mut ud = UserDict::new();
        ud.learn("aa", "一下");
        ud.learn("aa", "一下");
        let path = std::env::temp_dir().join(format!("hs_userdict_test_{}.json", std::process::id()));
        ud.save(&path).unwrap();
        let ud2 = UserDict::load(&path).unwrap();
        assert_eq!(ud2.count("aa", "一下"), 2);
        std::fs::remove_file(path).ok();
    }
}
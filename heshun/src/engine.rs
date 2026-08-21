//! 通用输入会话引擎：形码 + 音码，统一 Pipeline 架构。
//!
//! 支持三种输入方案：
//! - 形码（郑码）：TableTranslator → 前缀查表 → 满4码唯一自动上屏
//! - 全拼：ScriptTranslator → 分段 + DP组句 → 用户选词
//! - 自然码双拼：ScriptTranslator + Algebra → 双拼键→全拼 → DP组句
//!
//! 平台外壳只需要：调 Session::feed / select / backspace，
//! 读取 pending / candidates，消费 FeedResult。

use crate::composer::{self, SentenceCandidate};
use crate::dict::Dict;
use crate::pinyin::PinyinDict;
use crate::processor::Processor;
use crate::punctuator::Punctuator;
use crate::reverse_lookup::ReverseLookup;
use crate::user_dict::UserDict;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

// ── 公共类型 ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedResult {
    /// 已上屏，committed 为上屏文本
    Committed(String),
    /// 按键已接受，等待更多输入或用户选词
    Waiting,
    /// 按键被拒绝（非法字符、缓冲已满、或音码无匹配）
    Rejected,
}

/// 候选条目（统一形码/音码）
#[derive(Debug, Clone)]
pub struct Candidate {
    pub word: String,
    pub code: String,  // 形码=编码，音码=拼音
}

// ── 翻译器抽象 ─────────────────────────────────────────

/// 输入方案类型
pub enum SchemaKind {
    /// 形码方案（如郑码）
    Table {
        dict: Dict,
        max_code_len: usize,
        auto_select: bool,
        auto_select_pattern: Option<String>,
    },
    /// 音码方案（全拼/双拼）。双拼反向映射通过 PinyinDict.zrm() 获取。
    Script {
        dict: PinyinDict,
    },
}

// ── 引擎 ──────────────────────────────────────────────

pub struct Engine {
    schema: SchemaKind,
    /// 原方案的形码字典。反查时用于显示候选对应的形码编码。
    /// 仅 Table 方案保留；音码方案为 None。
    table_dict: Option<Dict>,
    /// 反查字典（如郑码按 ` 查拼音），None 表示不启用。
    pub reverse_lookup: Option<ReverseLookup>,
    /// 用户词典（选词学习 + 持久化）。
    pub user_dict: RefCell<Option<UserDict>>,
    /// 用户词典的绝对/已解析路径；退出时调用 save_user_dict 持久化。
    user_dict_path: Option<PathBuf>,
    /// 标点引擎（半角→全角映射），None 表示不转换标点。
    pub punctuator: Option<Punctuator>,
}

impl Engine {
    pub fn new(schema: SchemaKind) -> Engine {
        // 反查需要由字词反推形码；保留一份独立的形码索引。
        // Dict 没有 Clone，因此在从 schema 加载时会专门构建，手工构建 Engine 则不启用该注释。
        Engine {
            schema,
            table_dict: None,
            reverse_lookup: None,
            user_dict: RefCell::new(None),
            user_dict_path: None,
            punctuator: None,
        }
    }

    pub fn with_reverse_lookup(mut self, rl: ReverseLookup) -> Self {
        self.reverse_lookup = Some(rl);
        self
    }

    pub fn with_user_dict(mut self, ud: UserDict) -> Self {
        self.user_dict = RefCell::new(Some(ud));
        self
    }

    pub fn with_user_dict_path(mut self, path: PathBuf) -> Self {
        self.user_dict_path = Some(path);
        self
    }

    pub fn with_punctuator(mut self, p: Punctuator) -> Self {
        self.punctuator = Some(p);
        self
    }

    /// 用户词典写入路径（由 schema 配置，供 GUI/平台外壳在退出时保存）。
    pub fn user_dict_path(&self) -> Option<&Path> {
        self.user_dict_path.as_deref()
    }

    /// 保存当前用户词典；未启用用户词典时是成功的空操作。
    pub fn save_user_dict(&self) -> Result<(), String> {
        let Some(path) = self.user_dict_path() else { return Ok(()) };
        let borrowed = self.user_dict.borrow();
        let Some(dict) = borrowed.as_ref() else { return Ok(()) };
        dict.save(path).map_err(|e| format!("保存用户词典 {} 失败: {e}", path.display()))
    }

    /// 从 schema.yaml 加载引擎。
    /// 自动从 `dictionary.file` 加载码表二进制，识别 ZMD1/ZPY1 并配置引擎。
    pub fn from_schema_file(path: &std::path::Path) -> Result<Engine, String> {
        use crate::schema::SchemaConfig;

        let sc = SchemaConfig::load(path)?;
        let base = path.parent().unwrap_or(Path::new("."));
        // 字典文件路径：schema 同级目录 + dictionary.file
        let dict_path = base.join(&sc.dictionary.file);

        let data = std::fs::read(&dict_path)
            .map_err(|e| format!("读字典 {} 失败: {e}", dict_path.display()))?;
        if data.len() < 4 {
            return Err("字典文件太小".into());
        }
        let magic = u32::from_le_bytes(data[..4].try_into().unwrap());

        let default_code_len = sc.speller.max_code_length.unwrap_or(4);
        let auto_select = sc.speller.auto_select.unwrap_or(false);

        let mut table_dict = None;
        let kind = match magic {
            0x31444D5A => {
                let dict = crate::dict::Dict::load(&data)?;
                // 单独加载一份，供反查候选显示形码编码。
                table_dict = Some(crate::dict::Dict::load(&data)?);
                SchemaKind::Table {
                    dict,
                    max_code_len: default_code_len,
                    auto_select,
                    auto_select_pattern: sc.speller.auto_select_pattern.clone(),
                }
            }
            0x3159505A => {
                let dict = crate::pinyin::PinyinDict::load(&data)?;
                SchemaKind::Script { dict }
            }
            _ => return Err(format!("未知字典格式 (magic={magic:08X})")),
        };

        let mut engine = Engine::new(kind);
        engine.table_dict = table_dict;

        // 反查字典
        if !sc.reverse_lookup.file.is_empty() {
            let rl_path = base.join(&sc.reverse_lookup.file);
            let rl_data = std::fs::read(&rl_path)
                .map_err(|e| format!("读反查字典 {} 失败: {e}", rl_path.display()))?;
            if rl_data.len() >= 4 {
                let rl_magic = u32::from_le_bytes(rl_data[..4].try_into().unwrap());
                if rl_magic == 0x3159505A {
                    let rl_dict = crate::pinyin::PinyinDict::load(&rl_data)?;
                    let prefix = sc.reverse_lookup.prefix.as_deref().unwrap_or("`");
                    let prefix_char = prefix.chars().next().unwrap_or('`');
                    engine = engine.with_reverse_lookup(
                        crate::reverse_lookup::ReverseLookup::new(rl_dict, prefix_char)
                    );
                }
            }
        }

        // 标点引擎
        let full_shape = sc.punctuator.full_shape.unwrap_or(true);
        engine = engine.with_punctuator(Punctuator::new().with_full_shape(full_shape));

        // 用户词典
        if !sc.user_dict.file.is_empty() {
            let ud_path = base.join(&sc.user_dict.file);
            let ud = crate::user_dict::UserDict::load(&ud_path).unwrap_or_default();
            engine = engine.with_user_dict(ud);
            engine.user_dict_path = Some(ud_path);
        }

        Ok(engine)
    }

    pub fn session(&self) -> Session<'_> {
        Session {
            engine: self,
            buf: String::new(),
            // 音码候选缓存
            sentence_cands: Vec::new(),
            sentence_offset: 0,
            ascii_mode: false,
        }
    }

    pub fn is_table(&self) -> bool {
        matches!(self.schema, SchemaKind::Table { .. })
    }
}

// ── 会话 ──────────────────────────────────────────────

pub struct Session<'a> {
    engine: &'a Engine,
    buf: String,
    // 音码组句缓存
    sentence_cands: Vec<SentenceCandidate>,
    sentence_offset: usize,
    /// 是否处于西文模式（ascii_composer）
    pub ascii_mode: bool,
}

impl<'a> Session<'a> {
    /// 当前缓冲编码（预编辑显示）。音码显示原始拼音串。
    pub fn pending(&self) -> &str {
        &self.buf
    }

    /// 外部包装类型访问内部状态（供 GUI 等使用）。
    pub fn buf_mut(&mut self) -> &mut String { &mut self.buf }
    pub fn set_buf(&mut self, b: String) { self.buf = b; }
    pub fn set_sentence_cands(&mut self, sc: Vec<SentenceCandidate>) { self.sentence_cands = sc; }
    pub fn take_state(&mut self) -> (String, Vec<SentenceCandidate>) {
        (std::mem::take(&mut self.buf), std::mem::take(&mut self.sentence_cands))
    }
    pub fn restore_state(&mut self, buf: String, sc: Vec<SentenceCandidate>) {
        self.buf = buf;
        self.sentence_cands = sc;
    }

    /// 是否处于反查模式（pending 以反查前缀开头）。
    pub fn in_reverse_mode(&self) -> bool {
        self.engine.reverse_lookup.as_ref()
            .map(|rl| self.buf.starts_with(rl.prefix()))
            .unwrap_or(false)
    }

    /// 反查模式下的真实输入（去掉前缀后的查询串）。
    fn reverse_query(&self) -> Option<&str> {
        if let Some(rl) = &self.engine.reverse_lookup {
            if self.buf.starts_with(rl.prefix()) {
                return Some(&self.buf[rl.prefix().len_utf8()..]);
            }
        }
        None
    }

    /// 当前候选列表（最多 9 个，供序号选择）。
    pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
        if self.buf.is_empty() {
            return Vec::new();
        }

        // 反查模式：走反查字典
        if let Some(query) = self.reverse_query() {
            if let Some(rl) = &self.engine.reverse_lookup {
                let input = crate::pinyin::normalize_pinyin(query);
                let mut out = Vec::new();
                let limit = if limit == 0 { usize::MAX } else { limit };
                for cand in rl.dict().exact(&input) {
                    if out.len() >= limit { break; }
                    let codes = self.engine.table_dict.as_ref()
                        .map(|dict| dict.codes_for_word(&cand.word))
                        .unwrap_or_default();
                    let code = if codes.is_empty() {
                        "—".to_owned()
                    } else {
                        codes.join("/")
                    };
                    out.push(Candidate { word: cand.word, code });
                }
                for sc in &self.sentence_cands {
                    if out.len() >= limit { break; }
                    let w = sc.words.join("");
                    if out.iter().any(|c| c.word == w) { continue; }
                    let codes = self.engine.table_dict.as_ref()
                        .map(|dict| dict.codes_for_word(&w))
                        .unwrap_or_default();
                    let code = if codes.is_empty() { "—".to_owned() } else { codes.join("/") };
                    out.push(Candidate { word: w, code });
                }
                return out;
            }
        }
        let mut out = match &self.engine.schema {
            SchemaKind::Table { dict, .. } => {
                let cs = dict.prefix(&self.buf, if limit == 0 { 9 } else { limit });
                cs.iter().map(|c| Candidate {
                    word: c.word.to_string(), code: c.code.clone(),
                }).collect()
            }
            SchemaKind::Script { dict, .. } => {
                let mut out = Vec::new();
                let input = self.normalized_input();
                let limit = if limit == 0 { usize::MAX } else { limit };
                for cand in dict.exact(&input) {
                    if out.len() >= limit { break; }
                    out.push(Candidate { word: cand.word, code: input.clone() });
                }
                for sc in &self.sentence_cands {
                    if out.len() >= limit { break; }
                    let word = sc.words.join("");
                    if out.iter().any(|c| c.word == word) { continue; }
                    out.push(Candidate { word, code: input.clone() });
                }
                for cand in dict.prefix(&input) {
                    if out.len() >= limit { break; }
                    if out.iter().any(|c| c.word == cand.word) { continue; }
                    out.push(Candidate { word: cand.word, code: input.clone() });
                }
                out
            }
        };

        // 用户词典学习排序：在保持原有顺序的前提下提升已选过的候选。
        let learn_code = if self.in_reverse_mode() {
            self.reverse_query().unwrap_or("").to_string()
        } else {
            match &self.engine.schema {
                SchemaKind::Table { .. } => self.buf.clone(),
                SchemaKind::Script { .. } => self.normalized_input(),
            }
        };
        if let Some(ud) = self.engine.user_dict.borrow().as_ref() {
            ud.reorder_candidates(&learn_code, &mut out);
        }
        out
    }

    /// 输入一个键。处理顺序：
    /// 1. 反查模式：` 前缀触发，走反查字典
    /// 2. 西文模式：字母直接上屏
    /// 3. 标点：半角→全角转换
    /// 4. 正常编码（形码/音码）
    pub fn feed(&mut self, ch: char) -> FeedResult {
        // 反查模式：` 之后的字符走反查字典
        if self.in_reverse_mode() {
            return self.feed_reverse(ch);
        }

        // 反查前缀触发
        if self.buf.is_empty() {
            if let Some(rl) = &self.engine.reverse_lookup {
                if ch == rl.prefix() {
                    self.buf.push(ch);
                    return FeedResult::Waiting;
                }
            }
        }

        // 西文模式：字母直接上屏
        if self.ascii_mode {
            if ch.is_ascii_alphanumeric() || ch == ' ' {
                return FeedResult::Committed(ch.to_string());
            }
        }

        // 标点：半角→全角
        if let Some(p) = &self.engine.punctuator {
            let mut ctx = crate::processor::ProcessCtx::default();
            ctx.ascii_mode = self.ascii_mode;
            if let Some(outcome) = p.process(ch, &self.buf, &mut ctx) {
                if let crate::processor::ProcessOutcome::Handled(r) = outcome {
                    return r;
                }
            }
        }

        let c = ch.to_ascii_lowercase();
        if !c.is_ascii_alphabetic() {
            return FeedResult::Rejected;
        }
        match &self.engine.schema {
            SchemaKind::Table {
                dict,
                max_code_len,
                auto_select,
                ..
            } => self.feed_table(dict, c, *max_code_len, *auto_select),
            SchemaKind::Script { dict } => self.feed_script(dict, c),
        }
    }

    /// 反查模式按键：走反查字典（音码逻辑）。
    fn feed_reverse(&mut self, ch: char) -> FeedResult {
        let Some(rl) = &self.engine.reverse_lookup else {
            return FeedResult::Rejected;
        };
        let query = self.reverse_query().unwrap_or("").to_string();
        // 复用音码逻辑：把反查字典当作 script dict
        let c = ch.to_ascii_lowercase();
        if !c.is_ascii_alphabetic() {
            return FeedResult::Rejected;
        }
        self.buf.push(c);
        let q = query + &c.to_string();
        let dict = rl.dict();
        let input = crate::pinyin::normalize_pinyin(&q);
        let exact = dict.exact(&input);
        let prefix = dict.prefix(&input);
        if exact.is_empty() && prefix.is_empty() {
            self.buf.pop();
            return FeedResult::Rejected;
        }
        self.sentence_cands = composer::compose(&input, dict, 9);
        FeedResult::Waiting
    }

    // ── 形码模式 ──────────────────────────────────────────

    fn feed_table(
        &mut self,
        dict: &Dict,
        c: char,
        max_code_len: usize,
        auto_select: bool,
    ) -> FeedResult {
        if self.buf.len() >= max_code_len {
            return FeedResult::Rejected;
        }
        self.buf.push(c);

        if self.buf.len() == max_code_len && auto_select {
            let cands = dict.prefix(&self.buf, 2);
            if cands.is_empty() {
                self.buf.pop();
                return FeedResult::Rejected;
            }
            if cands.len() == 1 {
                let w = cands[0].word.to_string();
                self.buf.clear();
                return FeedResult::Committed(w);
            }
        }
        // 检查是否有候选（中间码也可能无候选）
        if dict.prefix(&self.buf, 1).is_empty() {
            self.buf.pop();
            return FeedResult::Rejected;
        }
        FeedResult::Waiting
    }

    // ── 音码模式 ──────────────────────────────────────────

    /// 将缓冲按键串转为连续全拼串。
    /// 全拼模式：直接去空格去引号；
    /// 双拼模式（zrm 存在）：先把双拼键映射回全拼，再去空格。
    fn normalized_input(&self) -> String {
        match &self.engine.schema {
            SchemaKind::Script { dict } => {
                let raw = &self.buf;
                if let Some(map) = dict.zrm() {
                    // 双拼：按键 → 全拼
                    map.to_pinyin(raw)
                } else {
                    crate::pinyin::normalize_pinyin(raw)
                }
            }
            _ => crate::pinyin::normalize_pinyin(&self.buf),
        }
    }

    fn feed_script(
        &mut self,
        dict: &PinyinDict,
        c: char,
    ) -> FeedResult {
        self.buf.push(c);

        let zrm = dict.zrm();
        // 双拼模式：按键串 → 全拼；全拼模式：直接归一化
        let input = if let Some(map) = zrm {
            map.to_pinyin(&self.buf)
        } else {
            self.normalized_input()
        };

        // 双拼下，末尾奇数键会被忽略（尚未构成完整音节），
        // 此时 input 可能为空或与上一状态相同——仍需接受按键。
        // 检查是否有任何候选（完整词或部分前缀）
        let exact = dict.exact(&input);
        let prefix = dict.prefix(&input);

        if input.is_empty() {
            // 双拼首个键尚未成音节：接受但无候选，等待下一键
            if zrm.is_some() && self.buf.chars().count() < 2 {
                return FeedResult::Waiting;
            }
            self.buf.pop();
            return FeedResult::Rejected;
        }

        if exact.is_empty() && prefix.is_empty() {
            // 双拼下允许中间态：即使当前全拼串无完整候选，仍可能继续输入
            if zrm.is_some() {
                return FeedResult::Waiting;
            }
            self.buf.pop();
            return FeedResult::Rejected;
        }

        // 运行 DP 组句获取整句候选
        self.sentence_cands = composer::compose(&input, dict, 9);
        self.sentence_offset = 0;

        FeedResult::Waiting
    }

    // ── 选词 / 控制 ────────────────────────────────────────

    /// 序号选词（1-based）。音码优先整句候选，其次单字。
    pub fn select(&mut self, idx: usize) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let idx = idx.wrapping_sub(1);
        let code_for_learn;
        // 候选顺序与 candidates() 保持一致
        let result = if self.in_reverse_mode() {
            let Some(rl) = &self.engine.reverse_lookup else { return None };
            let query = self.reverse_query().unwrap_or("");
            code_for_learn = query.to_string();
            let input = crate::pinyin::normalize_pinyin(query);
            let exact: Vec<_> = rl.dict().exact(&input);
            if idx < exact.len() {
                Some(exact[idx].word.clone())
            } else {
                let sent_offset = exact.len();
                if idx < sent_offset + self.sentence_cands.len() {
                    let sci = idx - sent_offset;
                    Some(self.sentence_cands[sci].words.join(""))
                } else {
                    None
                }
            }
        } else {
            match &self.engine.schema {
                SchemaKind::Table { dict, .. } => {
                    code_for_learn = self.buf.clone();
                    let cands = dict.prefix(&self.buf, 0);
                    cands.get(idx).map(|c| c.word.to_string())
                }
                SchemaKind::Script { dict, .. } => {
                    let input = self.normalized_input();
                    code_for_learn = input.clone();
                    let exact: Vec<_> = dict.exact(&input);
                    if idx < exact.len() {
                        Some(exact[idx].word.clone())
                    } else {
                        let sent_offset = exact.len();
                        if idx < sent_offset + self.sentence_cands.len() {
                            let sci = idx - sent_offset;
                            Some(self.sentence_cands[sci].words.join(""))
                        } else {
                            None
                        }
                    }
                }
            }
        };

        if let Some(ref w) = result {
            // 用户词典学习
            if let Some(ud) = self.engine.user_dict.borrow_mut().as_mut() {
                ud.learn(&code_for_learn, w);
            }
        }

        self.buf.clear();
        self.sentence_cands.clear();
        result
    }

    /// 直接按候选文本上屏。用于外壳在用户词典排序后按可见候选选择。
    pub fn select_word(&mut self, word: &str) -> Option<String> {
        let candidate = self.candidates(0).into_iter().find(|c| c.word == word)?;
        let code = if self.in_reverse_mode() {
            self.reverse_query().unwrap_or("").to_owned()
        } else {
            match &self.engine.schema {
                SchemaKind::Table { .. } => self.buf.clone(),
                SchemaKind::Script { .. } => self.normalized_input(),
            }
        };
        if let Some(ud) = self.engine.user_dict.borrow_mut().as_mut() {
            ud.learn(&code, &candidate.word);
        }
        self.buf.clear();
        self.sentence_cands.clear();
        Some(candidate.word)
    }

    /// 空格：首选上屏。
    pub fn select_first(&mut self) -> Option<String> {
        self.select(1)
    }

    /// 退格。
    pub fn backspace(&mut self) -> bool {
        self.sentence_cands.clear();
        self.buf.pop().is_some()
    }

    /// 清空缓冲。
    pub fn clear(&mut self) {
        self.buf.clear();
        self.sentence_cands.clear();
    }

    /// 外壳切焦点/模式时调用。
    pub fn flush(&mut self) -> Option<String> {
        self.sentence_cands.clear();
        if self.buf.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buf))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::encode_code;

    #[test]
    fn save_user_dict_uses_schema_relative_path() {
        let root = std::env::temp_dir().join(format!("hs_schema_userdict_{}", std::process::id()));
        let schemas = root.join("schemas");
        let data_dir = root.join("data");
        std::fs::create_dir_all(&schemas).unwrap();
        let dict_path = schemas.join("table.bin");
        let mut dict_data = Vec::new();
        Dict::from_entries(vec![(encode_code("a").unwrap(), "一".into())]).save(&mut dict_data).unwrap();
        std::fs::write(&dict_path, dict_data).unwrap();
        let schema_path = schemas.join("test.schema.yaml");
        std::fs::write(&schema_path, r#"
schema:
  schema_id: test
  name: Test
engine:
  type: table
dictionary:
  file: table.bin
user_dict:
  file: ../data/test.userdb.json
"#).unwrap();
        let engine = Engine::from_schema_file(&schema_path).unwrap();
        let mut session = engine.session();
        session.feed('a');
        session.select_first();
        engine.save_user_dict().unwrap();
        assert!(data_dir.join("test.userdb.json").is_file());
        std::fs::remove_dir_all(root).ok();
    }

    // ── 形码测试 ──────────────────────────────────────────

    fn table_engine() -> Engine {
        Engine::new(SchemaKind::Table {
            dict: Dict::from_entries(vec![
                (encode_code("j").unwrap(), "中".into()),
                (encode_code("ji").unwrap(), "虫".into()),
                (encode_code("jiv").unwrap(), "虽".into()),
                (encode_code("jivv").unwrap(), "中".into()),
                (encode_code("aa").unwrap(), "一下".into()),
            ]),
            max_code_len: 4,
            auto_select: true,
            auto_select_pattern: None,
        })
    }

    #[test]
    fn table_auto_commit_unique_4code() {
        let e = table_engine();
        let mut s = e.session();
        assert_eq!(s.feed('j'), FeedResult::Waiting);
        assert_eq!(s.feed('i'), FeedResult::Waiting);
        assert_eq!(s.feed('v'), FeedResult::Waiting);
        assert_eq!(s.feed('v'), FeedResult::Committed("中".into()));
    }

    #[test]
    fn table_invalid_4th_key_rejected() {
        let e = table_engine();
        let mut s = e.session();
        for c in ['j', 'i', 'v'] { s.feed(c); }
        assert_eq!(s.feed('z'), FeedResult::Rejected);
        assert_eq!(s.pending(), "jiv");
    }

    #[test]
    fn table_select_and_space() {
        let e = table_engine();
        let mut s = e.session();
        s.feed('j');
        assert_eq!(s.candidates(0).len(), 4);
        assert_eq!(s.select(2), Some("虫".into()));
        assert!(s.pending().is_empty());
    }

    // ── 音码测试 ──────────────────────────────────────────

    fn script_engine() -> Engine {
        Engine::new(SchemaKind::Script {
            dict: PinyinDict::from_entries(vec![
                ("wo".into(), "我".into(), 100),
                ("zhong".into(), "中".into(), 100),
                ("zhong".into(), "钟".into(), 80),
                ("zhong guo".into(), "中国".into(), 95),
                ("guo".into(), "国".into(), 90),
                ("guo".into(), "过".into(), 70),
            ]),
        })
    }

    #[test]
    fn script_basic_input() {
        let e = script_engine();
        let mut s = e.session();
        assert_eq!(s.feed('w'), FeedResult::Waiting);
        assert_eq!(s.feed('o'), FeedResult::Waiting);
        let cands = s.candidates(5);
        assert!(!cands.is_empty(), "应有候选");
        assert!(cands.iter().any(|c| c.word == "我"));

        let w = s.select_first();
        assert_eq!(w, Some("我".into()));
        assert!(s.pending().is_empty());
    }

    #[test]
    fn script_composition() {
        let e = script_engine();
        let mut s = e.session();
        for c in "zhonggu".chars() { s.feed(c); }
        // 此时应能匹配到 "中国"
        let cands = s.candidates(5);
        assert!(!cands.is_empty());
        // 清空
        s.clear();
        assert!(s.pending().is_empty());
    }

    #[test]
    fn script_invalid_key_rejected() {
        let e = script_engine();
        let mut s = e.session();
        s.feed('w'); // "w" 是 "wo" 前缀，有效
        assert_eq!(s.pending(), "w");
        assert_eq!(s.feed('x'), FeedResult::Rejected); // "wx" 无候选
        assert_eq!(s.pending(), "w"); // 已回退
    }

    #[test]
    fn backspace_and_clear_unified() {
        let e = script_engine();
        let mut s = e.session();
        s.feed('w'); s.feed('o');
        assert!(s.backspace());
        assert_eq!(s.pending(), "w");
        s.clear();
        assert_eq!(s.pending(), "");
    }
}
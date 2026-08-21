//! 代数规则引擎（对标 Rime speller.algebra + translator.preedit_format）
//!
//! 规则类型：
//! - xform/pattern/replacement/  正向变换（拼音 → 双拼键）
//! - derive/pattern/replacement/ 派生拼写（模糊匹配变体）
//! - abbrev/pattern/replacement/ 简拼（首字母匹配全拼）
//! - erase/pattern/              删除匹配
//! - xlit/from/to/               字符转写
//!
//! 用法：
//! - build 阶段：对词典中所有拼音应用 xform/xlit 得到双拼键 → 构建反向映射
//! - runtime 阶段：反向映射 + derive/abbrev 产生候选拼写

use regex::Regex;

/// 一条代数规则
#[derive(Debug, Clone)]
pub enum AlgebraRule {
    Xform {
        pattern: Regex,
        replacement: String,
    },
    Derive {
        pattern: Regex,
        replacement: String,
    },
    Abbrev {
        pattern: Regex,
        replacement: String,
    },
    Erase {
        pattern: Regex,
    },
    Xlit {
        from: String,
        to: String,
    },
}

/// 规则集合
#[derive(Debug)]
pub struct Algebra {
    rules: Vec<AlgebraRule>,
}

impl Algebra {
    pub fn new() -> Self {
        Algebra { rules: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// 从 Rime 格式的规则字符串数组构建
    ///
    /// 格式: "xform/pattern/replacement/" 或 "derive/pattern/replacement/"
    /// 其中 / 也可能被转义。
    ///
    /// 参数: raw_rules — 如 ["xform/iu$/Q/", "derive/^([nl])ue$/$1ve/"]
    pub fn from_strings(raw_rules: &[String]) -> Result<Self, String> {
        let mut rules = Vec::new();
        for raw in raw_rules {
            let rule = parse_rule(raw)?;
            rules.push(rule);
        }
        Ok(Algebra { rules })
    }

    /// 对输入字符串依次应用所有 xform/erase/xlit 规则，
    /// 返回变换后的字符串。用于 build 阶段计算双拼键。
    ///
    /// derive/abbrev 不在 transform 中执行——它们用于生成变体候选。
    pub fn transform(&self, input: &str) -> String {
        let mut s = input.to_string();
        for rule in &self.rules {
            match rule {
                AlgebraRule::Xform { pattern, replacement } => {
                    if pattern.is_match(&s) {
                        s = pattern.replace(&s, replacement.as_str()).to_string();
                    }
                }
                AlgebraRule::Erase { pattern } => {
                    if pattern.is_match(&s) {
                        return String::new(); // 被擦除 → 空
                    }
                }
                AlgebraRule::Xlit { from, to } => {
                    s = xlit(&s, from, to);
                }
                _ => {} // derive/abbrev 不参与 transform
            }
        }
        s
    }

    /// 对输入字符串产生派生拼写（只执行 derive 规则）。
    /// 返回所有变体（含原始输入）。
    pub fn derivations(&self, input: &str) -> Vec<String> {
        let mut results = vec![input.to_string()];
        for rule in &self.rules {
            if let AlgebraRule::Derive { pattern, replacement } = rule {
                let mut new = Vec::new();
                for s in &results {
                    if pattern.is_match(s) {
                        let derived = pattern.replace(s, replacement.as_str()).to_string();
                        if derived != *s {
                            new.push(derived);
                        }
                    }
                }
                results.extend(new);
            }
        }
        results
    }

    /// 对输入产生简拼形式（只执行 abbrev 规则）。
    pub fn abbreviations(&self, input: &str) -> Vec<String> {
        let mut results = Vec::new();
        for rule in &self.rules {
            if let AlgebraRule::Abbrev { pattern, replacement } = rule {
                if pattern.is_match(input) {
                    let abbr = pattern.replace(input, replacement.as_str()).to_string();
                    if abbr != input {
                        results.push(abbr);
                    }
                }
            }
        }
        results
    }

    /// 对输入执行 preedit_format 变换（显示用）。
    /// 这是 translator.preedit_format 的等价——把内部表示变成显示格式。
    /// 与 algebra 共享规则语法，但语义不同（应用于显示时）。
    pub fn preedit_format(&self, input: &str) -> String {
        self.transform(input)
    }

    /// 访问原始规则（供 build 阶段复用）
    pub fn rules(&self) -> &[AlgebraRule] {
        &self.rules
    }

    /// 自然码双拼（zrm）预设规则。来自官方 double_pinyin.schema.yaml 的 speller.algebra。
    /// 用于 build 阶段：全拼音节 → 双拼键（每音节恰好 2 键）。
    pub fn natural_code() -> Self {
        let rules = [
            "erase/^xx$/",
            "derive/^([jqxy])u$/$1v/",
            "derive/^([aoe])([ioun])$/$1$1$2/",
            "xform/^([aoe])(ng)?$/$1$1$2/",
            "xform/iu$/Q/",
            "xform/[iu]a$/W/",
            "xform/[uv]an$/R/",
            "xform/[uv]e$/T/",
            "xform/ing$|uai$/Y/",
            "xform/^sh/U/",
            "xform/^ch/I/",
            "xform/^zh/V/",
            "xform/uo$/O/",
            "xform/[uv]n$/P/",
            "xform/i?ong$/S/",
            "xform/[iu]ang$/D/",
            "xform/(.)en$/$1F/",
            "xform/(.)eng$/$1G/",
            "xform/(.)ang$/$1H/",
            "xform/ian$/M/",
            "xform/(.)an$/$1J/",
            "xform/iao$/C/",
            "xform/(.)ao$/$1K/",
            "xform/(.)ai$/$1L/",
            "xform/(.)ei$/$1Z/",
            "xform/ie$/X/",
            "xform/ui$/V/",
            "xform/(.)ou$/$1B/",
            "xform/in$/N/",
            "xlit/QWRTYUIOPSDFGHMJCKLZXVBN/qwrtyuiopsdfghmjcklzxvbn/",
        ];
        Algebra::from_strings(
            &rules.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
        .expect("自然码预设规则应能解析")
    }
}

/// 字符转写：from 中每个字符映射到 to 中对应位置的字符
fn xlit(s: &str, from: &str, to: &str) -> String {
    if from.len() != to.len() {
        return s.to_string();
    }
    let map: Vec<(char, char)> = from.chars().zip(to.chars()).collect();
    s.chars()
        .map(|c| map.iter().find(|(f, _)| *f == c).map(|(_, t)| *t).unwrap_or(c))
        .collect()
}

/// 解析单条规则字符串
fn parse_rule(raw: &str) -> Result<AlgebraRule, String> {
    // Rime 规则格式: type/pattern/replacement/
    // 分隔符是 / ，但在字符类 [...] 内部的 / 不算分隔符

    let (rule_type, rest) = raw
        .split_once('/')
        .ok_or_else(|| format!("无效规则（缺少 /）: {raw}"))?;

    match rule_type {
        "erase" => {
            let pattern = rest.trim_end_matches('/');
            let re = Regex::new(pattern)
                .map_err(|e| format!("正则 '{pattern}' 无效: {e}"))?;
            Ok(AlgebraRule::Erase { pattern: re })
        }
        "xlit" => {
            // xlit/from/to/
            let parts: Vec<&str> = rest.splitn(2, '/').collect();
            if parts.len() < 2 {
                return Err(format!("xlit 缺少分割: {raw}"));
            }
            let from = parts[0].to_string();
            let to = parts[1].trim_end_matches('/').to_string();
            if from.len() != to.len() {
                return Err(format!("xlit from/to 长度不等: {raw}"));
            }
            Ok(AlgebraRule::Xlit { from, to })
        }
        "xform" | "derive" | "abbrev" => {
            // 从 rest 中拆分 pattern 和 replacement
            let (pattern_str, replacement) = split_pattern_replacement(rest)?;
            let re = Regex::new(&pattern_str)
                .map_err(|e| format!("正则 '{pattern_str}' 无效: {e}"))?;
            // Rime 用 $1, $2 表示捕获组，但 Rust regex crate 的 $1F 会被解析为
            // 不存在的组引用（空字符串），而非 $1+字面 F。需要规范化为 ${1} 形式。
            let replacement = normalize_replacement(replacement.trim_end_matches('/'));
            match rule_type {
                "xform" => Ok(AlgebraRule::Xform {
                    pattern: re,
                    replacement,
                }),
                "derive" => Ok(AlgebraRule::Derive {
                    pattern: re,
                    replacement,
                }),
                "abbrev" => Ok(AlgebraRule::Abbrev {
                    pattern: re,
                    replacement,
                }),
                _ => unreachable!(),
            }
        }
        _ => Err(format!("未知规则类型: {rule_type}")),
    }
}

/// 从 "pattern/replacement/" 中拆分 pattern 和 replacement。
///
/// Rime 规则格式: type/pattern/replacement/
/// pattern 中不含顶层 `/`（字符类 [..] 内的 `/` 不计），
/// replacement 中也不含 `/`，末尾的 `/` 是终止符。
/// 所以正确分隔符是第一个（depth 0 的）`/`。
fn split_pattern_replacement(rest: &str) -> Result<(String, String), String> {
    let chars: Vec<char> = rest.chars().collect();
    let mut depth = 0i32;
    let mut split_pos = None;

    for (i, &ch) in chars.iter().enumerate() {
        match ch {
            '[' => depth += 1,
            ']' => depth -= 1,
            '/' if depth == 0 => {
                split_pos = Some(i);
                break; // 第一个顶层 / 即分隔符
            }
            _ => {}
        }
    }

    let split_pos = split_pos.ok_or_else(|| format!("无法分割 pattern/replacement: {rest}"))?;
    let pattern = chars[..split_pos].iter().collect::<String>();
    // replacement = 分隔符之后到末尾（去掉终止的 /）
    let replacement = chars[split_pos + 1..]
        .iter()
        .collect::<String>()
        .trim_end_matches('/')
        .to_string();

    Ok((pattern, replacement))
}

/// 将 Rime 风格的捕获组引用 ($1, $2) 规范化为 Rust regex crate 兼容格式。
///
/// Rust regex crate 的 replacement 语法: $N 仅匹配单数字，且 $1F 会被当成
/// 不存在的组 "1F"（返回空字符串）。Rime 的 $1F 语义是「第1组+字面 F」。
/// 此函数将所有 $N 转换为 ${N}，消除歧义。
fn normalize_replacement(repl: &str) -> String {
    let chars: Vec<char> = repl.chars().collect();
    let mut out = String::with_capacity(repl.len() + 4);
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            // 收集连续数字
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            let digits: String = chars[start..end].iter().collect();
            out.push('$');
            out.push('{');
            out.push_str(&digits);
            out.push('}');
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xlit() {
        assert_eq!(xlit("abcABC", "abc", "xyz"), "xyzABC");
        assert_eq!(xlit("hello", "a", "b"), "hello"); // 找不到→不变
    }

    #[test]
    fn test_parse_xform() {
        let rules =
            Algebra::from_strings(&["xform/iu$/Q/".into(), "xform/^zh/V/".into()]).unwrap();
        assert_eq!(rules.rules().len(), 2);
        // 拼音→双拼键
        assert_eq!(rules.transform("jiu"), "jQ");
        assert_eq!(rules.transform("zhong"), "Vong");
    }

    #[test]
    fn test_parse_erase() {
        let rules = Algebra::from_strings(&["erase/^xx$/".into()]).unwrap();
        assert_eq!(rules.transform("xx"), "");
        assert_eq!(rules.transform("xi"), "xi");
    }

    #[test]
    fn test_parse_derive() {
        let rules =
            Algebra::from_strings(&["derive/^([nl])ue$/$1ve/".into()]).unwrap();
        let derivs = rules.derivations("nue");
        assert!(derivs.contains(&"nve".to_string()));
    }

    #[test]
    fn test_parse_abbrev() {
        let rules =
            Algebra::from_strings(&["abbrev/^([a-z]).+$/$1/".into()]).unwrap();
        let abbrs = rules.abbreviations("zhong");
        assert_eq!(abbrs, vec!["z"]);
    }

    #[test]
    fn test_xform_with_capture() {
        let rules =
            Algebra::from_strings(&["xform/(.)en$/$1F/".into()]).unwrap();
        assert_eq!(rules.transform("ben"), "bF");
        assert_eq!(rules.transform("zhen"), "zhF");
    }

    #[test]
    fn test_xlit_rule() {
        let rules =
            Algebra::from_strings(&["xlit/abc/xyz/".into()]).unwrap();
        assert_eq!(rules.transform("abc"), "xyz");
    }

    #[test]
    fn test_double_pinyin_sample() {
        // 自然码双拼核心规则
        let rules = Algebra::from_strings(&[
            "erase/^xx$/".into(),
            "derive/^([jqxy])u$/$1v/".into(),
            "derive/^([aoe])([ioun])$/$1$1$2/".into(),
            "xform/^([aoe])(ng)?$/$1$1$2/".into(),
            "xform/iu$/Q/".into(),
            "xform/[iu]a$/W/".into(),
            "xform/[uv]an$/R/".into(),
            "xform/[uv]e$/T/".into(),
            "xform/ing$|uai$/Y/".into(),
            "xform/^sh/U/".into(),
            "xform/^ch/I/".into(),
            "xform/^zh/V/".into(),
            "xform/uo$/O/".into(),
        ])
        .unwrap();

        // 中 zhong → VS (V=zh, S=ong)
        assert_eq!(rules.transform("zhong"), "Vong"); // ^zh→V, ong 还没处理
        // 需要组合规则: ^zh→V 且 ong$→S
        // 规则顺序执行: 先 ^zh→V 把 "zhong"→"Vong"，然后检查 ong$ 规则
        // 但这需要规则 "xform/i?ong$/S/" 在 xform/^zh/ 之后
        // 实际自然码双拼是 all rules applied sequentially
    }

    #[test]
    fn test_full_natural_double_pinyin() {
        // 完整自然码双拼 algebra
        let rules = Algebra::from_strings(&[
            "erase/^xx$/".into(),
            "derive/^([jqxy])u$/$1v/".into(),
            "derive/^([aoe])([ioun])$/$1$1$2/".into(),
            "xform/^([aoe])(ng)?$/$1$1$2/".into(),
            "xform/iu$/Q/".into(),
            "xform/[iu]a$/W/".into(),
            "xform/[uv]an$/R/".into(),
            "xform/[uv]e$/T/".into(),
            "xform/ing$|uai$/Y/".into(),
            "xform/^sh/U/".into(),
            "xform/^ch/I/".into(),
            "xform/^zh/V/".into(),
            "xform/uo$/O/".into(),
            "xform/[uv]n$/P/".into(),
            "xform/i?ong$/S/".into(),
            "xform/[iu]ang$/D/".into(),
            "xform/(.)en$/$1F/".into(),
            "xform/(.)eng$/$1G/".into(),
            "xform/(.)ang$/$1H/".into(),
            "xform/ian$/M/".into(),
            "xform/(.)an$/$1J/".into(),
            "xform/iao$/C/".into(),
            "xform/(.)ao$/$1K/".into(),
            "xform/(.)ai$/$1L/".into(),
            "xform/(.)ei$/$1Z/".into(),
            "xform/ie$/X/".into(),
            "xform/ui$/V/".into(),
            "xform/(.)ou$/$1B/".into(),
            "xform/in$/N/".into(),
            "xlit/QWRTYUIOPSDFGHMJCKLZXVBN/qwrtyuiopsdfghmjcklzxvbn/".into(),
        ])
        .unwrap();

        // 中 zhong: ^zh→V, ong$→S → VS → vs (xlit 小写)
        let result = rules.transform("zhong");
        assert_eq!(result, "vs", "zhong → vs");

        // 国 guo: uo$→O → gO → go
        assert_eq!(rules.transform("guo"), "go", "guo → go");

        // 输 shu: ^sh→U, no ending match → Uu → uu
        // Actually shu → Uu, but xlit lowercase: uu.
        // But double pinyin for "shu" is indeed "uu"!
        assert_eq!(rules.transform("shu"), "uu", "shu → uu");

        // 入 ru: no match → ru
        assert_eq!(rules.transform("ru"), "ru", "ru → ru");
    }
}
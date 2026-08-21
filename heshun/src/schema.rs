//! 方案配置层（schema）：对标 Rime 的 `*.schema.yaml`。
//!
//! 一份 schema 声明「用什么字典 + 引擎怎么跑」。换方案 = 换 schema.yaml，
//! 无需重新编译 Rust。
//!
//! schema.yaml 示例（形码）：
//! ```yaml
//! schema:
//!   schema_id: zhengma66
//!   name: 郑码6.6
//!   version: "6.6"
//! engine:
//!   type: table                # table(形码) | script(音码)
//! dictionary:
//!   file: zhengma.bin          # 编译后的二进制（运行时加载）
//! speller:
//!   max_code_length: 4
//!   auto_select: true
//!   auto_select_pattern: "^[a-z]{4}$"
//! ```
//!
//! 音码 + 双拼示例：`speller.algebra` 声明双拼键位规则（build 阶段据此构建
//! ZRM1 反向映射，运行时二进制已内嵌，无需再解析 algebra）。

use serde::Deserialize;
use std::path::Path;

/// 顶层 schema 配置。
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaConfig {
    pub schema: SchemaMeta,
    pub engine: EngineSection,
    #[serde(default)]
    pub dictionary: DictionarySection,
    #[serde(default)]
    pub speller: SpellerSection,
    #[serde(default)]
    pub reverse_lookup: ReverseLookupSection,
    #[serde(default)]
    pub punctuator: PunctuatorSection,
    #[serde(default)]
    pub user_dict: UserDictSection,
}

/// schema 元信息。
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaMeta {
    pub schema_id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
}

/// 引擎段。
#[derive(Debug, Clone, Deserialize)]
pub struct EngineSection {
    /// "table"（形码）| "script"（音码）
    #[serde(rename = "type")]
    pub engine_type: String,
}

/// 字典段。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DictionarySection {
    /// 编译后的二进制路径（运行时加载）。
    #[serde(default)]
    pub file: String,
    /// 源码表路径（build 阶段用，如 郑码6.6.txt / xxx.dict.yaml）。
    #[serde(default)]
    pub source: String,
}

/// 拼写段（speller）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpellerSection {
    #[serde(default)]
    pub max_code_length: Option<usize>,
    #[serde(default)]
    pub auto_select: Option<bool>,
    #[serde(default)]
    pub auto_select_pattern: Option<String>,
    #[serde(default)]
    pub alphabet: Option<String>,
    #[serde(default)]
    pub algebra: Option<Vec<String>>,
}

/// 反查段（reverse_lookup）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReverseLookupSection {
    /// 反查字典（如 "pinyin_simp"）。
    #[serde(default)]
    pub dictionary: String,
    /// 反查字典二进制文件路径。
    #[serde(default)]
    pub file: String,
    /// 触发前缀（默认 "`"）。
    #[serde(default)]
    pub prefix: Option<String>,
}

/// 标点段（punctuator）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PunctuatorSection {
    /// 是否启用全角标点（默认 true）。
    #[serde(default)]
    pub full_shape: Option<bool>,
}

/// 用户词典段（user_dict）。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct UserDictSection {
    /// 持久化文件路径（如 "zhengma66.userdb.json"）。
    #[serde(default)]
    pub file: String,
}

impl SchemaConfig {
    /// 从 YAML 文件加载 schema。
    pub fn load(path: &Path) -> Result<SchemaConfig, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("读 schema 失败 {}: {e}", path.display()))?;
        Self::parse(&text)
    }

    /// 从 YAML 文本解析。
    pub fn parse(text: &str) -> Result<SchemaConfig, String> {
        serde_yaml::from_str(text).map_err(|e| format!("解析 schema YAML 失败: {e}"))
    }

    /// 引擎类型是否为音码（script）。
    pub fn is_script(&self) -> bool {
        self.engine.engine_type == "script"
    }

    /// 引擎类型是否为形码（table）。
    pub fn is_table(&self) -> bool {
        self.engine.engine_type == "table"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table_schema() {
        let yaml = r#"
schema:
  schema_id: zhengma66
  name: 郑码6.6
  version: "6.6"
engine:
  type: table
dictionary:
  file: zhengma.bin
speller:
  max_code_length: 4
  auto_select: true
  auto_select_pattern: "^[a-z]{4}$"
"#;
        let c = SchemaConfig::parse(yaml).unwrap();
        assert_eq!(c.schema.schema_id, "zhengma66");
        assert!(c.is_table());
        assert_eq!(c.dictionary.file, "zhengma.bin");
        assert_eq!(c.speller.max_code_length, Some(4));
        assert_eq!(c.speller.auto_select, Some(true));
    }

    #[test]
    fn parse_script_schema_with_algebra() {
        let yaml = r#"
schema:
  schema_id: double_pinyin_zrm
  name: 自然码双拼
engine:
  type: script
dictionary:
  file: pinyin_zrm.bin
speller:
  algebra:
    - erase/^xx$/
    - xform/^zh/V/
    - xform/iu$/Q/
"#;
        let c = SchemaConfig::parse(yaml).unwrap();
        assert!(c.is_script());
        let alg = c.speller.algebra.as_ref().unwrap();
        assert_eq!(alg.len(), 3);
        assert_eq!(alg[0], "erase/^xx$/");
    }

    #[test]
    fn parse_minimal_schema() {
        // 缺省字段（dictionary/speller）应为默认值
        let yaml = r#"
schema:
  schema_id: foo
  name: Foo
engine:
  type: script
"#;
        let c = SchemaConfig::parse(yaml).unwrap();
        assert_eq!(c.dictionary.file, "");
        assert_eq!(c.speller.max_code_length, None);
    }
}
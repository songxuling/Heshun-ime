//! heshun — 通用中文输入法引擎核心
//!
//! 全平台共享的输入解码核心：
//! - [`algebra`] 代数规则引擎（双拼键位映射）
//! - [`dict`] 形码码表
//! - [`pinyin`] 音码字典
//! - [`zrm`] 双拼反向映射
//! - [`composer`] 拼音 DP 组句
//! - [`engine`] 统一会话引擎
//! - [`processor`] 处理器骨架
//! - [`punctuator`] 标点引擎
//! - [`reverse_lookup`] 反查
//! - [`user_dict`] 用户词典
//! - [`schema`] 方案配置层

pub mod algebra;
pub mod composer;
pub mod context_score;
pub mod core;
pub mod dict;
pub mod engine;
pub mod ffi;
pub mod history;
pub mod pinyin;
pub mod processor;
pub mod punctuator;
pub mod reverse_lookup;
pub mod schema;
pub mod segmentation;
pub mod scorer;
pub mod translation;
pub mod user_dict;
pub mod word_graph;
pub mod zrm;

pub use algebra::Algebra;
pub use composer::compose;
pub use core::{CandidateKey, CandidatePage, CandidateSource, CandidateView, CommandResult, ContextSnapshot, CoreError, CoreRuntime, CoreState, EngineStore, EventDisposition, InputEvent, RuntimeStatus, SchemaId, Segment};
pub use dict::Dict;
pub use engine::{Candidate, Engine, FeedResult, SchemaKind, Session};
pub use pinyin::PinyinDict;
pub use schema::SchemaConfig;
pub use segmentation::{EdgeProperties, SpellingType, SyllableEdge, SyllableGraph};
pub use zrm::ZrmMap;
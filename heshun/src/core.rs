//! Owned, platform-neutral input runtime.
//!
//! This is the boundary for GUI and native shells. It owns session state and
//! keeps the current borrowed Engine API behind a short-lived internal adapter.

use crate::composer::SentenceCandidate;
use crate::engine::{Engine, FeedResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub type SchemaId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandidateSource {
    Table = 1,
    ScriptExact = 2,
    ScriptSentence = 3,
    ScriptPrefix = 4,
    Reverse = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CandidateKey {
    pub source: CandidateSource,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateView {
    pub key: CandidateKey,
    pub word: String,
    pub annotation: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePage {
    pub items: Vec<CandidateView>,
    pub page_index: usize,
    pub page_size: usize,
    pub total: usize,
    pub selected: Option<CandidateKey>,
    pub has_previous: bool,
    pub has_next: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreState {
    pub schema: SchemaId,
    pub pending: String,
    pub sentence_candidates: Vec<SentenceCandidate>,
    pub ascii_mode: bool,
    pub full_shape: bool,
    pub page_index: usize,
    pub selected: Option<CandidateKey>,
}

impl CoreState {
    pub fn new(schema: impl Into<SchemaId>) -> Self {
        Self {
            schema: schema.into(),
            pending: String::new(),
            sentence_candidates: Vec::new(),
            ascii_mode: false,
            full_shape: true,
            page_index: 0,
            selected: None,
        }
    }

    fn clear_composition(&mut self) {
        self.pending.clear();
        self.sentence_candidates.clear();
        self.page_index = 0;
        self.selected = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    Text(char),
    Backspace,
    Delete,
    Escape,
    Space,
    Enter,
    Select(CandidateKey),
    MoveSelection(i32),
    Page(i32),
    ToggleAscii,
    SetAscii(bool),
    ToggleFullShape,
    SetSchema(SchemaId),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventDisposition {
    Consumed,
    PassedThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositionAction {
    Keep,
    Update,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub schema: SchemaId,
    pub ascii_mode: bool,
    pub full_shape: bool,
    pub composing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub pending: String,
    pub candidates: CandidatePage,
    pub status: RuntimeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreError {
    UnknownSchema(SchemaId),
    InvalidCandidate(CandidateKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub disposition: EventDisposition,
    pub committed: Option<String>,
    pub composition: CompositionAction,
    pub snapshot: ContextSnapshot,
    pub error: Option<CoreError>,
}

pub struct EngineStore {
    schemas: HashMap<SchemaId, Arc<Engine>>,
}

impl EngineStore {
    pub fn new() -> Self {
        Self { schemas: HashMap::new() }
    }

    pub fn insert(&mut self, id: impl Into<SchemaId>, engine: Engine) {
        self.schemas.insert(id.into(), Arc::new(engine));
    }

    pub fn get(&self, id: &str) -> Option<&Arc<Engine>> {
        self.schemas.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.schemas.contains_key(id)
    }
}

impl Default for EngineStore {
    fn default() -> Self { Self::new() }
}

pub struct CoreRuntime {
    store: Arc<EngineStore>,
    state: CoreState,
    page_size: usize,
}

impl CoreRuntime {
    pub fn new(store: Arc<EngineStore>, schema: impl Into<SchemaId>) -> Result<Self, CoreError> {
        let schema = schema.into();
        if !store.contains(&schema) {
            return Err(CoreError::UnknownSchema(schema));
        }
        Ok(Self { store, state: CoreState::new(schema), page_size: 9 })
    }

    pub fn state(&self) -> &CoreState { &self.state }
    pub fn snapshot(&self) -> ContextSnapshot { self.build_snapshot() }

    pub fn schema_id(&self) -> &str { &self.state.schema }

    pub fn save_user_dict(&self) -> Result<(), String> {
        let Some(engine) = self.store.get(&self.state.schema) else {
            return Err(format!("unknown schema: {}", self.state.schema));
        };
        engine.save_user_dict()
    }

    pub fn save_user_dict_to(&self, path: &std::path::Path) -> Result<(), String> {
        let Some(engine) = self.store.get(&self.state.schema) else {
            return Err(format!("unknown schema: {}", self.state.schema));
        };
        engine.save_user_dict_to(path)
    }

    pub fn dispatch(&mut self, event: InputEvent) -> CommandResult {
        let mut committed = None;
        let mut error = None;
        let mut disposition = EventDisposition::Consumed;
        let mut composition = if self.state.pending.is_empty() {
            CompositionAction::End
        } else {
            CompositionAction::Keep
        };

        match event {
            InputEvent::Text(ch) => {
                self.reset_candidate_view();
                let Some(engine) = self.store.get(&self.state.schema).cloned() else {
                    error = Some(CoreError::UnknownSchema(self.state.schema.clone()));
                    return self.result(disposition, committed, CompositionAction::End, error);
                };
                let mut session = engine.session();
                session.restore_state(self.state.pending.clone(), self.state.sentence_candidates.clone());
                session.ascii_mode = self.state.ascii_mode;
                if let FeedResult::Committed(text) = session.feed(ch) { committed = Some(text); }
                self.save_session(&mut session);
                composition = if self.state.pending.is_empty() { CompositionAction::End } else { CompositionAction::Update };
            }
            InputEvent::Backspace => {
                if self.state.pending.is_empty() {
                    return self.result(EventDisposition::PassedThrough, None, CompositionAction::End, None);
                }
                self.with_session(|session| { session.backspace(); });
                self.reset_candidate_view();
                composition = if self.state.pending.is_empty() { CompositionAction::End } else { CompositionAction::Update };
            }
            InputEvent::Delete => disposition = EventDisposition::PassedThrough,
            InputEvent::Escape | InputEvent::Reset => {
                self.state.clear_composition();
                composition = CompositionAction::End;
            }
            InputEvent::Space | InputEvent::Enter => {
                if let Some(key) = self.selected_or_first() {
                    committed = self.select_key(key, &mut error);
                    if committed.is_some() {
                        self.state.clear_composition();
                        composition = CompositionAction::End;
                    }
                } else {
                    // A candidate command without a candidate must not destroy
                    // an editable full-Pinyin composition.
                    composition = if self.state.pending.is_empty() {
                        CompositionAction::End
                    } else {
                        CompositionAction::Keep
                    };
                }
            }
            InputEvent::Select(key) => {
                committed = self.select_key(key, &mut error);
                if committed.is_some() { self.state.clear_composition(); composition = CompositionAction::End; }
            }
            InputEvent::MoveSelection(delta) => {
                let page = self.build_snapshot().candidates;
                if !page.items.is_empty() {
                    let current = page.selected.and_then(|key| page.items.iter().position(|item| item.key == key)).unwrap_or(0);
                    let next = (current as i32 + delta).rem_euclid(page.items.len() as i32) as usize;
                    self.state.selected = Some(page.items[next].key);
                }
            }
            InputEvent::Page(delta) => self.move_page(delta),
            InputEvent::ToggleAscii => self.set_ascii(!self.state.ascii_mode),
            InputEvent::SetAscii(value) => self.set_ascii(value),
            InputEvent::ToggleFullShape => self.state.full_shape = !self.state.full_shape,
            InputEvent::SetSchema(schema) => {
                if self.store.contains(&schema) {
                    self.state.schema = schema;
                    self.state.clear_composition();
                    composition = CompositionAction::End;
                } else {
                    error = Some(CoreError::UnknownSchema(schema));
                }
            }
        }
        self.result(disposition, committed, composition, error)
    }

    fn result(&self, disposition: EventDisposition, committed: Option<String>, composition: CompositionAction, error: Option<CoreError>) -> CommandResult {
        CommandResult { disposition, committed, composition, snapshot: self.build_snapshot(), error }
    }

    fn set_ascii(&mut self, value: bool) {
        self.state.ascii_mode = value;
        self.state.clear_composition();
    }

    fn reset_candidate_view(&mut self) {
        self.state.page_index = 0;
        self.state.selected = None;
    }

    fn selected_or_first(&self) -> Option<CandidateKey> {
        let page = self.build_snapshot().candidates;
        page.selected.or_else(|| page.items.first().map(|item| item.key))
    }

    fn select_key(&mut self, key: CandidateKey, error: &mut Option<CoreError>) -> Option<String> {
        let all = self.all_candidates();
        if !all.iter().any(|item| item.key == key) {
            *error = Some(CoreError::InvalidCandidate(key));
            return None;
        }
        let word = all.iter().find(|item| item.key == key)?.word.clone();
        self.with_session(|session| session.select_word(&word))
    }

    fn move_page(&mut self, delta: i32) {
        let total = self.build_snapshot().candidates.total;
        let pages = total.div_ceil(self.page_size);
        let next = (self.state.page_index as i32 + delta).clamp(0, pages.saturating_sub(1) as i32) as usize;
        self.state.page_index = next;
        self.state.selected = None;
    }

    fn with_session<T>(&mut self, f: impl FnOnce(&mut crate::engine::Session<'_>) -> T) -> T {
        let engine = self.store.get(&self.state.schema).cloned().expect("validated schema");
        let mut session = engine.session();
        session.restore_state(self.state.pending.clone(), self.state.sentence_candidates.clone());
        session.ascii_mode = self.state.ascii_mode;
        let result = f(&mut session);
        self.save_session(&mut session);
        result
    }

    fn save_session(&mut self, session: &mut crate::engine::Session<'_>) {
        let (pending, sentence) = session.take_state();
        self.state.pending = pending;
        self.state.sentence_candidates = sentence;
    }

    fn build_snapshot(&self) -> ContextSnapshot {
        let all = self.all_candidates();
        let total = all.len();
        let start = (self.state.page_index * self.page_size).min(total);
        let end = (start + self.page_size).min(total);
        let items = all[start..end].to_vec();
        let selected = self.state.selected.filter(|key| items.iter().any(|item| item.key == *key));
        ContextSnapshot {
            pending: self.state.pending.clone(),
            candidates: CandidatePage {
                items,
                page_index: self.state.page_index,
                page_size: self.page_size,
                total,
                selected,
                has_previous: start > 0,
                has_next: end < total,
            },
            status: RuntimeStatus {
                schema: self.state.schema.clone(),
                ascii_mode: self.state.ascii_mode,
                full_shape: self.state.full_shape,
                composing: !self.state.pending.is_empty(),
            },
        }
    }

    fn all_candidates(&self) -> Vec<CandidateView> {
        let Some(engine) = self.store.get(&self.state.schema) else { return Vec::new() };
        let mut session = engine.session();
        session.restore_state(self.state.pending.clone(), self.state.sentence_candidates.clone());
        session.ascii_mode = self.state.ascii_mode;
        session.candidates(usize::MAX).into_iter().enumerate().map(|(ordinal, candidate)| CandidateView {
            key: CandidateKey { source: if engine.is_table() { CandidateSource::Table } else { CandidateSource::ScriptExact }, ordinal: ordinal as u32 },
            annotation: candidate.code,
            word: candidate.word,
            label: (ordinal % self.page_size + 1).to_string(),
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::{encode_code, Dict};
    use crate::engine::SchemaKind;
    use crate::pinyin::PinyinDict;

    fn store() -> Arc<EngineStore> {
        let mut store = EngineStore::new();
        store.insert("table", Engine::new(SchemaKind::Table {
            dict: Dict::from_entries(vec![(encode_code("a").unwrap(), "一".into()), (encode_code("ab").unwrap(), "丁".into())]),
            max_code_len: 4, auto_select: false, auto_select_pattern: None,
        }));
        store.insert("script", Engine::new(SchemaKind::Script { dict: PinyinDict::from_entries(vec![
            ("wo".into(), "我".into(), 100),
            ("zhong".into(), "中".into(), 100),
        ]) }));
        Arc::new(store)
    }

    #[test]
    fn owned_runtime_switches_schema_without_borrowed_state() {
        let mut runtime = CoreRuntime::new(store(), "table").unwrap();
        runtime.dispatch(InputEvent::Text('a'));
        assert_eq!(runtime.snapshot().status.schema, "table");
        let result = runtime.dispatch(InputEvent::SetSchema("script".into()));
        assert_eq!(result.error, None);
        assert_eq!(result.snapshot.status.schema, "script");
        assert!(result.snapshot.pending.is_empty());
    }

    #[test]
    fn candidate_key_selects_the_displayed_candidate() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        runtime.dispatch(InputEvent::Text('w'));
        runtime.dispatch(InputEvent::Text('o'));
        let page = runtime.snapshot().candidates;
        let first = page.items[0].clone();
        let result = runtime.dispatch(InputEvent::Select(first.key));
        assert_eq!(result.committed, Some(first.word));
    }

    #[test]
    fn unmatched_full_pinyin_remains_editable_without_old_candidates() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() { runtime.dispatch(InputEvent::Text(ch)); }
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.pending, "wox");
        assert!(snapshot.candidates.items.is_empty());
        assert!(snapshot.status.composing);
    }

    #[test]
    fn invalid_candidate_key_preserves_composition() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wo".chars() { runtime.dispatch(InputEvent::Text(ch)); }
        let before = runtime.snapshot();
        let invalid = CandidateKey { source: CandidateSource::Table, ordinal: 999 };
        let result = runtime.dispatch(InputEvent::Select(invalid));
        assert_eq!(result.committed, None);
        assert_eq!(result.error, Some(CoreError::InvalidCandidate(invalid)));
        assert_eq!(result.snapshot.pending, before.pending);
        assert!(result.snapshot.status.composing);
    }

    #[test]
    fn paged_candidate_key_selects_from_full_candidate_set() {
        let mut store = EngineStore::new();
        let entries = (0..20)
            .map(|i| (crate::dict::encode_code("a").unwrap(), format!("字{i}")))
            .collect();
        store.insert("table", Engine::new(SchemaKind::Table {
            dict: Dict::from_entries(entries),
            max_code_len: 4,
            auto_select: false,
            auto_select_pattern: None,
        }));
        let mut runtime = CoreRuntime::new(Arc::new(store), "table").unwrap();
        runtime.dispatch(InputEvent::Text('a'));
        runtime.dispatch(InputEvent::Page(1));
        let page = runtime.snapshot().candidates;
        let target = page.items[0].clone();
        let result = runtime.dispatch(InputEvent::Select(target.key));
        assert_eq!(result.committed, Some(target.word));
    }

    #[test]
    fn event_replay_roundtrip_preserves_snapshot() {
        let events = vec![
            InputEvent::Text('w'),
            InputEvent::Text('o'),
            InputEvent::MoveSelection(1),
            InputEvent::Backspace,
            InputEvent::Text('o'),
        ];
        let encoded = serde_json::to_string(&events).unwrap();
        let decoded: Vec<InputEvent> = serde_json::from_str(&encoded).unwrap();

        let store = store();
        let mut first = CoreRuntime::new(store.clone(), "script").unwrap();
        let mut replay = CoreRuntime::new(store, "script").unwrap();
        for event in events { first.dispatch(event); }
        for event in decoded { replay.dispatch(event); }
        assert_eq!(first.snapshot(), replay.snapshot());
    }

    #[test]
    fn empty_candidate_command_preserves_pending() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        let result = runtime.dispatch(InputEvent::Space);
        assert_eq!(result.committed, None);
        assert_eq!(result.disposition, EventDisposition::Consumed);
        assert_eq!(result.snapshot.pending, "wox");
        assert!(result.snapshot.status.composing);
    }
}

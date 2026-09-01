//! Owned, platform-neutral input runtime.
//!
//! This is the boundary for GUI and native shells. It owns session state and
//! keeps the current borrowed Engine API behind a short-lived internal adapter.

use crate::composer::SentenceCandidate;
use crate::engine::{Engine, FeedResult};
use crate::history::{CommitHistory, CommitRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// A TSF runtime result is copied across the C ABI and then rendered by a
// native candidate window. Keep that payload bounded even when a dictionary
// has a very large exact-match bucket.
const MAX_SNAPSHOT_CANDIDATES: usize = 64;

pub type SchemaId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandidateSource {
    Table = 1,
    TableCompletion = 6,
    ScriptExact = 2,
    ScriptSentence = 3,
    ScriptPrefix = 4,
    Reverse = 5,
    ScriptAbbreviation = 7,
    ScriptCorrection = 8,
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
pub struct Segment {
    pub input: String,
    pub text: Option<String>,
    pub confirmed: bool,
    pub cursor: usize,
    pub page_index: usize,
    pub selected: Option<CandidateKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreState {
    pub schema: SchemaId,
    pub segments: Vec<Segment>,
    pub current_segment: usize,
    /// 已确认的前置片段，保留在当前组合态中。
    pub confirmed_text: String,
    pub pending: String,
    /// 插入光标在 pending 中的 Unicode 字符索引。
    pub cursor: usize,
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
            segments: Vec::new(),
            current_segment: 0,
            confirmed_text: String::new(),
            pending: String::new(),
            cursor: 0,
            sentence_candidates: Vec::new(),
            ascii_mode: false,
            full_shape: true,
            page_index: 0,
            selected: None,
        }
    }

    fn clear_composition(&mut self) {
        self.pending.clear();
        self.cursor = 0;
        self.sentence_candidates.clear();
        self.page_index = 0;
        self.selected = None;
    }

    fn rebuild_confirmed_text(&mut self) {
        self.confirmed_text = self
            .segments
            .iter()
            .filter(|segment| segment.confirmed)
            .filter_map(|segment| segment.text.as_deref())
            .collect();
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
    MoveCursor(i32),
    Page(i32),
    ToggleAscii,
    SetAscii(bool),
    ToggleFullShape,
    SetSchema(SchemaId),
    Reset,
    ConfirmSegment,
    ReopenPreviousSegment,
    RevertLastEdit,
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
    pub segments: Vec<Segment>,
    pub current_segment: usize,
    pub confirmed_text: String,
    pub pending: String,
    pub cursor: usize,
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
        Self {
            schemas: HashMap::new(),
        }
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
    fn default() -> Self {
        Self::new()
    }
}

pub struct CoreRuntime {
    store: Arc<EngineStore>,
    state: CoreState,
    page_size: usize,
    history: CommitHistory,
    preceding_text: String,
}

impl CoreRuntime {
    pub fn new(store: Arc<EngineStore>, schema: impl Into<SchemaId>) -> Result<Self, CoreError> {
        let schema = schema.into();
        if !store.contains(&schema) {
            return Err(CoreError::UnknownSchema(schema));
        }
        Ok(Self {
            store,
            state: CoreState::new(schema),
            page_size: 9,
            history: CommitHistory::new(32),
            preceding_text: String::new(),
        })
    }

    pub fn state(&self) -> &CoreState {
        &self.state
    }
    pub fn snapshot(&self) -> ContextSnapshot {
        self.build_snapshot()
    }

    pub fn schema_id(&self) -> &str {
        &self.state.schema
    }

    pub fn commit_history(&self) -> &CommitHistory {
        &self.history
    }

    pub fn confirmed_text(&self) -> &str {
        &self.state.confirmed_text
    }

    /// 设置宿主提供的前文，用于 Script 上下文评分。
    pub fn set_preceding_text(&mut self, text: impl Into<String>) {
        self.preceding_text = text.into();
    }

    pub fn preceding_text(&self) -> &str {
        &self.preceding_text
    }

    pub fn edit_snapshot(&self) -> CoreState {
        self.state.clone()
    }

    pub fn restore_edit_snapshot(&mut self, snapshot: CoreState) -> bool {
        if !self.store.contains(&snapshot.schema) {
            return false;
        }
        self.state = snapshot;
        self.state.rebuild_confirmed_text();
        if self.state.segments.is_empty() {
            self.state.current_segment = 0;
            self.state.clear_composition();
            return true;
        }
        self.state.current_segment = self
            .state
            .current_segment
            .min(self.state.segments.len() - 1);
        self.focus_segment(self.state.current_segment)
    }

    pub fn current_segment_index(&self) -> usize {
        self.state.current_segment
    }

    pub fn current_segment(&self) -> Option<&Segment> {
        self.state.segments.get(self.state.current_segment)
    }

    pub fn focus_segment(&mut self, index: usize) -> bool {
        if index >= self.state.segments.len() {
            return false;
        }
        self.state.current_segment = index;
        if let Some(segment) = self.state.segments.get(index).cloned() {
            self.state.pending = segment.input;
            self.state.cursor = segment.cursor.min(self.state.pending.chars().count());
            self.state.page_index = segment.page_index;
            self.state.selected = segment.selected;
        }
        true
    }

    pub fn move_segment(&mut self, delta: i32) -> bool {
        if self.state.segments.is_empty() {
            return false;
        }
        let next = (self.state.current_segment as i32 + delta)
            .clamp(0, self.state.segments.len().saturating_sub(1) as i32) as usize;
        self.focus_segment(next)
    }

    pub fn delete_segment(&mut self, index: usize) -> bool {
        if index >= self.state.segments.len() {
            return false;
        }
        let _segment = self.state.segments.remove(index);
        self.state.rebuild_confirmed_text();
        if self.state.current_segment >= self.state.segments.len() {
            self.state.current_segment = self.state.segments.len().saturating_sub(1);
        }
        if let Some(current) = self.state.segments.get(self.state.current_segment).cloned() {
            self.state.pending = current.input;
            self.state.cursor = current.cursor;
            self.state.page_index = current.page_index;
            self.state.selected = current.selected;
        } else {
            self.state.clear_composition();
        }
        true
    }

    pub fn revert_last_edit(&mut self) -> bool {
        let Some(record) = self.history.pop() else {
            return false;
        };
        if self.state.confirmed_text.ends_with(&record.text) {
            let new_len = self.state.confirmed_text.len() - record.text.len();
            self.state.confirmed_text.truncate(new_len);
        }
        if record.learned {
            if let Some(engine) = self.store.get(&self.state.schema) {
                if let Some(user_dict) = engine.user_dict.borrow_mut().as_mut() {
                    return user_dict.forget(&record.code, &record.text);
                }
            }
        }
        true
    }

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
                session.restore_state_at(
                    self.state.pending.clone(),
                    self.state.sentence_candidates.clone(),
                    self.state.cursor,
                );
                session.ascii_mode = self.state.ascii_mode;
        session.set_preceding_word(
            self.preceding_text
                .chars()
                .last()
                .or_else(|| self.state.confirmed_text.chars().last())
                .map(|c| c.to_string()),
        );
                if let FeedResult::Committed(text) = session.feed_at_cursor(ch) {
                    committed = Some(text);
                }
                self.save_session(&mut session);
                composition = if self.state.pending.is_empty() {
                    CompositionAction::End
                } else {
                    CompositionAction::Update
                };
            }
            InputEvent::Backspace => {
                if self.state.pending.is_empty()
                    && self.state.current_segment > 0
                {
                    let _ = self.focus_segment(self.state.current_segment - 1);
                    self.state.cursor = self.state.pending.chars().count();
                }
                if self.state.pending.is_empty() {
                    if self.revert_last_edit() {
                        self.reset_candidate_view();
                        composition = if self.state.pending.is_empty() {
                            CompositionAction::End
                        } else {
                            CompositionAction::Update
                        };
                        return self.result(
                            disposition,
                            committed,
                            composition,
                            error,
                        );
                    }
                    return self.result(
                        EventDisposition::PassedThrough,
                        None,
                        CompositionAction::End,
                        None,
                    );
                }
                if self.state.cursor == 0 && self.state.current_segment > 0 {
                    let _ = self.focus_segment(self.state.current_segment - 1);
                    self.state.cursor = self.state.pending.chars().count();
                }
                self.with_session(|session| {
                    session.backspace();
                });
                if self.state.pending.is_empty() && self.state.segments.len() > 1 {
                    let _ = self.delete_segment(self.state.current_segment);
                }
                self.reset_candidate_view();
                composition = if self.state.pending.is_empty() {
                    CompositionAction::End
                } else {
                    CompositionAction::Update
                };
            }
            InputEvent::Delete => {
                if self.delete_at_cursor() {
                    composition = CompositionAction::Update;
                } else {
                    disposition = EventDisposition::PassedThrough;
                }
            }
            InputEvent::Escape | InputEvent::Reset => {
                self.state.clear_composition();
                self.state.confirmed_text.clear();
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
                if committed.is_some() {
                    self.state.clear_composition();
                    composition = CompositionAction::End;
                }
            }
            InputEvent::ConfirmSegment => {
                if let Some(key) = self.selected_or_first() {
                    if self.select_key(key, &mut error).is_some() {
                        committed = None;
                        composition = CompositionAction::Keep;
                    }
                }
            }
            InputEvent::ReopenPreviousSegment => {
                if let Some(record) = self.history.pop() {
                    if self.state.confirmed_text.ends_with(&record.text) {
                        let new_len = self.state.confirmed_text.len() - record.text.len();
                        self.state.confirmed_text.truncate(new_len);
                    }
                    self.state.segments.pop();
                    self.state.current_segment = self.state.segments.len();
                    self.state.pending = record.code;
                    self.state.cursor = self.state.pending.chars().count();
                    self.reset_candidate_view();
                    composition = CompositionAction::Update;
                }
            }
            InputEvent::RevertLastEdit => {
                if self.revert_last_edit() {
                    composition = CompositionAction::Update;
                }
            }
            InputEvent::MoveSelection(delta) => {
                let page = self.build_snapshot().candidates;
                if !page.items.is_empty() {
                    let current = page
                        .selected
                        .and_then(|key| page.items.iter().position(|item| item.key == key))
                        .unwrap_or(0);
                    let next =
                        (current as i32 + delta).rem_euclid(page.items.len() as i32) as usize;
                    self.state.selected = Some(page.items[next].key);
                    self.sync_current_segment_state();
                }
            }
            InputEvent::MoveCursor(delta) => {
                self.move_cursor_across_segments(delta);
                composition = if self.state.pending.is_empty() {
                    CompositionAction::End
                } else {
                    CompositionAction::Keep
                };
            }
            InputEvent::Page(delta) => {
                self.move_page(delta);
                self.sync_current_segment_state();
            }
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

    fn result(
        &self,
        disposition: EventDisposition,
        committed: Option<String>,
        composition: CompositionAction,
        error: Option<CoreError>,
    ) -> CommandResult {
        CommandResult {
            disposition,
            committed,
            composition,
            snapshot: self.build_snapshot(),
            error,
        }
    }

    fn delete_at_cursor(&mut self) -> bool {
        let len = self.state.pending.chars().count();
        if self.state.cursor < len {
            let mut chars: Vec<char> = self.state.pending.chars().collect();
            chars.remove(self.state.cursor);
            self.state.pending = chars.into_iter().collect();
            self.state.sentence_candidates.clear();
            self.state.page_index = 0;
            self.state.selected = None;
            self.sync_current_segment_state();
            return true;
        }
        if self.state.current_segment + 1 < self.state.segments.len() {
            self.sync_current_segment_state();
            return self.delete_segment(self.state.current_segment + 1);
        }
        false
    }

    fn move_cursor_across_segments(&mut self, delta: i32) {
        if delta < 0 && self.state.cursor == 0 && self.state.current_segment > 0 {
            let _ = self.focus_segment(self.state.current_segment - 1);
            self.state.cursor = self.state.pending.chars().count();
        } else if delta > 0
            && self.state.cursor >= self.state.pending.chars().count()
            && self.state.current_segment + 1 < self.state.segments.len()
        {
            let _ = self.focus_segment(self.state.current_segment + 1);
            self.state.cursor = 0;
        } else {
            self.state.cursor = (self.state.cursor as i32 + delta)
                .clamp(0, self.state.pending.chars().count() as i32)
                as usize;
        }
        self.sync_current_segment_state();
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
        page.selected
            .or_else(|| page.items.first().map(|item| item.key))
    }

    fn select_key(&mut self, key: CandidateKey, error: &mut Option<CoreError>) -> Option<String> {
        let all = self.all_candidates();
        if !all.iter().any(|item| item.key == key) {
            *error = Some(CoreError::InvalidCandidate(key));
            return None;
        }
        let word = all.iter().find(|item| item.key == key)?.word.clone();
        let code = self.state.pending.clone();
        if let Some(engine) = self.store.get(&self.state.schema) {
            if let Some(user_dict) = engine.user_dict.borrow_mut().as_mut() {
                user_dict.begin_transaction();
            }
        }
        let result = self.with_session(|session| session.select_word(&word));
        if let Some(engine) = self.store.get(&self.state.schema) {
            if let Some(user_dict) = engine.user_dict.borrow_mut().as_mut() {
                if result.is_some() {
                    if !user_dict.commit_transaction() {
                        user_dict.rollback_transaction();
                    }
                } else {
                    user_dict.rollback_transaction();
                }
            }
        }
        if let Some(text) = result.as_ref() {
            let segment = Segment {
                input: code.clone(),
                text: Some(text.clone()),
                confirmed: true,
                cursor: 0,
                page_index: 0,
                selected: Some(key),
            };
            if self.state.current_segment < self.state.segments.len()
                && !self.state.segments[self.state.current_segment].confirmed
            {
                self.state.segments[self.state.current_segment] = segment;
            } else {
                self.state.segments.push(segment);
                self.state.current_segment = self.state.segments.len() - 1;
            }
            self.state.rebuild_confirmed_text();
            self.history.push(CommitRecord {
                text: text.clone(),
                code,
                learned: true,
            });
        }
        result
    }

    fn move_page(&mut self, delta: i32) {
        let total = self.build_snapshot().candidates.total;
        let pages = total.div_ceil(self.page_size);
        let next = (self.state.page_index as i32 + delta).clamp(0, pages.saturating_sub(1) as i32)
            as usize;
        self.state.page_index = next;
        self.state.selected = None;
    }

    fn with_session<T>(&mut self, f: impl FnOnce(&mut crate::engine::Session<'_>) -> T) -> T {
        let engine = self
            .store
            .get(&self.state.schema)
            .cloned()
            .expect("validated schema");
        let mut session = engine.session();
        self.load_current_segment_state();
        session.restore_state_at(
            self.state.pending.clone(),
            self.state.sentence_candidates.clone(),
            self.state.cursor,
        );
        session.ascii_mode = self.state.ascii_mode;
        session.set_preceding_word(
            self.preceding_text
                .chars()
                .last()
                .or_else(|| self.state.confirmed_text.chars().last())
                .map(|c| c.to_string()),
        );
        let result = f(&mut session);
        self.save_session(&mut session);
        result
    }

    fn save_session(&mut self, session: &mut crate::engine::Session<'_>) {
        let (pending, sentence, cursor) = session.take_state();
        self.state.pending = pending;
        self.state.sentence_candidates = sentence;
        self.state.cursor = cursor;
        self.sync_current_segment_state();
    }

    fn load_current_segment_state(&mut self) {
        if let Some(segment) = self
            .state
            .segments
            .get(self.state.current_segment)
            .cloned()
            .filter(|segment| !segment.confirmed)
        {
            self.state.pending = segment.input;
            self.state.cursor = segment.cursor;
            self.state.page_index = segment.page_index;
            self.state.selected = segment.selected;
        }
    }

    fn sync_current_segment_state(&mut self) {
        if self.state.pending.is_empty() {
            if self.state.current_segment < self.state.segments.len()
                && !self.state.segments[self.state.current_segment].confirmed
            {
                self.state.segments.remove(self.state.current_segment);
                self.state.current_segment = self
                    .state
                    .current_segment
                    .min(self.state.segments.len().saturating_sub(1));
                self.state.rebuild_confirmed_text();
            }
            return;
        }
        let segment = Segment {
            input: self.state.pending.clone(),
            text: None,
            confirmed: false,
            cursor: self.state.cursor,
            page_index: self.state.page_index,
            selected: self.state.selected,
        };
        if self.state.current_segment < self.state.segments.len()
            && !self.state.segments[self.state.current_segment].confirmed
        {
            self.state.segments[self.state.current_segment] = segment;
        } else {
            self.state.segments.push(segment);
            self.state.current_segment = self.state.segments.len() - 1;
        }
    }

    fn build_snapshot(&self) -> ContextSnapshot {
        let all = self.all_candidates();
        let total = all.len();
        let start = (self.state.page_index * self.page_size).min(total);
        let end = (start + self.page_size).min(total);
        let items = all[start..end].to_vec();
        let selected = self
            .state
            .selected
            .filter(|key| items.iter().any(|item| item.key == *key));
        ContextSnapshot {
            segments: self.state.segments.clone(),
            current_segment: self.state.current_segment,
            confirmed_text: self.state.confirmed_text.clone(),
            pending: self.state.pending.clone(),
            cursor: self.state.cursor,
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
        let Some(engine) = self.store.get(&self.state.schema) else {
            return Vec::new();
        };
        let mut session = engine.session();
        session.restore_state_at(
            self.state.pending.clone(),
            self.state.sentence_candidates.clone(),
            self.state.cursor,
        );
        session.ascii_mode = self.state.ascii_mode;
        session.set_preceding_word(
            self.preceding_text
                .chars()
                .last()
                .or_else(|| self.state.confirmed_text.chars().last())
                .map(|c| c.to_string()),
        );
        session
            .candidates(MAX_SNAPSHOT_CANDIDATES)
            .into_iter()
            .enumerate()
            .map(|(ordinal, candidate)| CandidateView {
                key: CandidateKey {
                    source: candidate.source,
                    ordinal: ordinal as u32,
                },
                annotation: candidate.code,
                word: candidate.word,
                label: (ordinal % self.page_size + 1).to_string(),
            })
            .collect()
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
        store.insert(
            "table",
            Engine::new(SchemaKind::Table {
                dict: Dict::from_entries(vec![
                    (encode_code("a").unwrap(), "一".into()),
                    (encode_code("ab").unwrap(), "丁".into()),
                ]),
                max_code_len: 4,
                auto_select: false,
                auto_select_pattern: None,
            }),
        );
        store.insert(
            "script",
            Engine::new(SchemaKind::Script {
                dict: PinyinDict::from_entries(vec![
                    ("wo".into(), "我".into(), 100),
                    ("zhong".into(), "中".into(), 100),
                ]),
            })
            .with_user_dict(crate::user_dict::UserDict::new()),
        );
        Arc::new(store)
    }

    #[test]
    fn owned_runtime_switches_schema_without_borrowed_state() {
        let mut runtime = CoreRuntime::new(store(), "table").unwrap();
        runtime.set_preceding_text("前文");
        assert_eq!(runtime.preceding_text(), "前文");
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
        assert_eq!(runtime.commit_history().len(), 1);
        assert_eq!(runtime.commit_history().last().unwrap().text, "我");
        assert_eq!(runtime.confirmed_text(), "我");
    }

    #[test]
    fn confirm_segment_keeps_confirmed_text_and_revert_restores_pending() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        runtime.dispatch(InputEvent::Text('w'));
        runtime.dispatch(InputEvent::Text('o'));
        let result = runtime.dispatch(InputEvent::ConfirmSegment);
        assert_eq!(result.committed, None);
        assert_eq!(runtime.confirmed_text(), "我");
        assert!(runtime.state().pending.is_empty());
        assert!(runtime.dispatch(InputEvent::ReopenPreviousSegment).snapshot.pending == "wo");
        assert_eq!(runtime.confirmed_text(), "");
    }

    #[test]
    fn script_snapshot_preserves_candidate_origins() {
        let mut store = EngineStore::new();
        store.insert(
            "script",
            Engine::new(SchemaKind::Script {
                dict: PinyinDict::from_entries(vec![
                    ("zhong".into(), "中".into(), 100),
                    ("guo".into(), "国".into(), 90),
                ]),
            }),
        );
        let mut runtime = CoreRuntime::new(Arc::new(store), "script").unwrap();
        for ch in "zhongguo".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        let page = runtime.snapshot().candidates;
        assert!(
            page.items
                .iter()
                .any(|item| item.key.source == CandidateSource::ScriptSentence),
            "组句候选必须保留 ScriptSentence 来源"
        );
    }

    #[test]
    fn unmatched_full_pinyin_remains_editable_without_old_candidates() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.pending, "wox");
        assert!(snapshot.candidates.items.is_empty());
        assert!(snapshot.status.composing);
    }

    #[test]
    fn script_candidate_snapshot_has_a_bounded_total() {
        let mut store = EngineStore::new();
        let entries = (0..80)
            .map(|i| ("a".to_string(), format!("候选{i}"), 80 - i))
            .collect();
        store.insert(
            "script",
            Engine::new(SchemaKind::Script {
                dict: PinyinDict::from_entries(entries),
            }),
        );
        let mut runtime = CoreRuntime::new(Arc::new(store), "script").unwrap();
        runtime.dispatch(InputEvent::Text('a'));
        let snapshot = runtime.snapshot();
        assert!(snapshot.candidates.total <= MAX_SNAPSHOT_CANDIDATES);
    }

    #[test]
    fn long_full_pinyin_stays_editable_while_candidates_are_suppressed() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        let input = "a".repeat(65);
        for ch in input.chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.pending, input);
        assert!(snapshot.candidates.items.is_empty());
        assert_eq!(snapshot.candidates.total, 0);
        assert!(snapshot.status.composing);
    }

    #[test]
    fn invalid_candidate_key_preserves_composition() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wo".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        let before = runtime.snapshot();
        let invalid = CandidateKey {
            source: CandidateSource::Table,
            ordinal: 999,
        };
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
        store.insert(
            "table",
            Engine::new(SchemaKind::Table {
                dict: Dict::from_entries(entries),
                max_code_len: 4,
                auto_select: false,
                auto_select_pattern: None,
            }),
        );
        let mut runtime = CoreRuntime::new(Arc::new(store), "table").unwrap();
        runtime.dispatch(InputEvent::Text('a'));
        runtime.dispatch(InputEvent::Page(1));
        let page = runtime.snapshot().candidates;
        let target = page.items[0].clone();
        let result = runtime.dispatch(InputEvent::Select(target.key));
        assert_eq!(result.committed, Some(target.word));
    }

    #[test]
    fn cursor_moves_and_inserts_at_middle() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        assert_eq!(runtime.snapshot().cursor, 3);
        runtime.dispatch(InputEvent::MoveCursor(-2));
        assert_eq!(runtime.snapshot().cursor, 1);
        runtime.dispatch(InputEvent::Text('a'));
        assert_eq!(runtime.snapshot().pending, "waox");
        assert_eq!(runtime.snapshot().cursor, 2);
    }

    #[test]
    fn cursor_backspace_deletes_before_cursor() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        runtime.dispatch(InputEvent::MoveCursor(-1));
        runtime.dispatch(InputEvent::Backspace);
        assert_eq!(runtime.snapshot().pending, "wx");
        assert_eq!(runtime.snapshot().cursor, 1);
    }

    #[test]
    fn cursor_backspace_at_middle_updates_segment_state() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        runtime.dispatch(InputEvent::MoveCursor(-1));
        runtime.dispatch(InputEvent::Backspace);
        assert_eq!(runtime.snapshot().segments.last().unwrap().input, "wx");
    }

    #[test]
    fn current_segment_can_move_and_delete() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wo".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        runtime.dispatch(InputEvent::ConfirmSegment);
        for ch in "zhong".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        runtime.dispatch(InputEvent::ConfirmSegment);
        assert_eq!(runtime.current_segment_index(), 1);
        assert!(runtime.move_segment(-1));
        assert_eq!(runtime.current_segment_index(), 0);
        assert_eq!(runtime.current_segment().unwrap().input, "wo");
        assert!(runtime.delete_segment(0));
        assert_eq!(runtime.current_segment_index(), 0);
        assert_eq!(runtime.current_segment().unwrap().input, "zhong");
    }

    #[test]
    fn current_segment_focuses_and_keeps_cursor() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        runtime.dispatch(InputEvent::MoveCursor(-1));
        assert!(runtime.focus_segment(0));
        assert_eq!(runtime.snapshot().cursor, 2);
    }

    #[test]
    fn current_segment_rejects_out_of_range_focus() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        assert!(!runtime.focus_segment(1));
    }

    #[test]
    fn current_segment_move_handles_empty_state() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        assert!(!runtime.move_segment(1));
    }

    #[test]
    fn commit_failure_rolls_back_user_dict_learning() {
        let store = store();
        {
            let engine = store.get("script").unwrap();
            engine
                .user_dict
                .borrow_mut()
                .as_mut()
                .unwrap()
                .fail_next_commit_once();
        }
        let mut runtime = CoreRuntime::new(store.clone(), "script").unwrap();
        for ch in "wo".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        let first = runtime.snapshot().candidates.items[0].clone();
        assert_eq!(runtime.dispatch(InputEvent::Select(first.key)).committed, Some(first.word));
        let engine = store.get("script").unwrap();
        assert_eq!(engine.user_dict.borrow().as_ref().unwrap().count("wo", "我"), 0);
    }

    #[test]
    fn cursor_clamps_at_pending_boundaries() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        runtime.dispatch(InputEvent::MoveCursor(-1));
        assert_eq!(runtime.snapshot().cursor, 0);
        for ch in "wo".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        runtime.dispatch(InputEvent::MoveCursor(99));
        assert_eq!(runtime.snapshot().cursor, 2);
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
        for event in events {
            first.dispatch(event);
        }
        for event in decoded {
            replay.dispatch(event);
        }
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

    #[test]
    fn deleting_middle_segment_rebuilds_confirmed_text() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        runtime.state.segments = vec![
            Segment { input: "a".into(), text: Some("甲".into()), confirmed: true, cursor: 0, page_index: 0, selected: None },
            Segment { input: "b".into(), text: Some("乙".into()), confirmed: true, cursor: 0, page_index: 0, selected: None },
            Segment { input: "c".into(), text: Some("丙".into()), confirmed: true, cursor: 0, page_index: 0, selected: None },
        ];
        runtime.state.current_segment = 1;
        runtime.state.rebuild_confirmed_text();
        assert!(runtime.delete_segment(1));
        assert_eq!(runtime.confirmed_text(), "甲丙");
        assert_eq!(runtime.snapshot().segments.len(), 2);
    }

    #[test]
    fn delete_at_cursor_removes_current_character_and_resets_menu() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        for ch in "wox".chars() {
            runtime.dispatch(InputEvent::Text(ch));
        }
        runtime.dispatch(InputEvent::MoveCursor(-1));
        let result = runtime.dispatch(InputEvent::Delete);
        assert_eq!(result.snapshot.pending, "wo");
        assert_eq!(result.snapshot.cursor, 2);
        assert_eq!(result.snapshot.candidates.page_index, 0);
    }

    #[test]
    fn cursor_moves_across_segment_boundaries() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        runtime.state.segments = vec![
            Segment { input: "wo".into(), text: None, confirmed: false, cursor: 0, page_index: 0, selected: None },
            Segment { input: "zhong".into(), text: None, confirmed: false, cursor: 0, page_index: 0, selected: None },
        ];
        runtime.state.current_segment = 1;
        runtime.focus_segment(1);
        runtime.dispatch(InputEvent::MoveCursor(-99));
        assert_eq!(runtime.current_segment_index(), 0);
        assert_eq!(runtime.snapshot().cursor, 2);
        runtime.dispatch(InputEvent::MoveCursor(99));
        assert_eq!(runtime.current_segment_index(), 1);
        assert_eq!(runtime.snapshot().cursor, 0);
    }

    #[test]
    fn cross_segment_backspace_and_delete_update_segment_list() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        runtime.state.segments = vec![
            Segment { input: "wo".into(), text: None, confirmed: false, cursor: 2, page_index: 0, selected: None },
            Segment { input: "zhong".into(), text: None, confirmed: false, cursor: 0, page_index: 0, selected: None },
        ];
        runtime.state.current_segment = 1;
        runtime.focus_segment(1);
        runtime.dispatch(InputEvent::Backspace);
        assert_eq!(runtime.current_segment_index(), 0);
        assert_eq!(runtime.snapshot().pending, "w");
        runtime.state.current_segment = 0;
        runtime.state.pending = "wo".into();
        runtime.state.cursor = 2;
        runtime.dispatch(InputEvent::Delete);
        assert_eq!(runtime.snapshot().segments.len(), 1);
        assert_eq!(runtime.snapshot().pending, "wo");
    }

    #[test]
    fn edit_snapshot_restores_segments_cursor_and_menu() {
        let mut runtime = CoreRuntime::new(store(), "script").unwrap();
        runtime.state.segments = vec![
            Segment { input: "wo".into(), text: None, confirmed: false, cursor: 1, page_index: 2, selected: None },
        ];
        runtime.state.current_segment = 0;
        runtime.focus_segment(0);
        let snapshot = runtime.edit_snapshot();
        runtime.dispatch(InputEvent::Text('x'));
        assert!(runtime.restore_edit_snapshot(snapshot));
        assert_eq!(runtime.snapshot().pending, "wo");
        assert_eq!(runtime.snapshot().cursor, 1);
        assert_eq!(runtime.snapshot().candidates.page_index, 2);
    }
}

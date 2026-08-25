//! C FFI：供各平台外壳链接（Windows TSF C++ / macOS IMK ObjC / Android JNI / iOS）。
//!
//! 全部函数 `#[no_mangle] extern "C"`，字符串入参为 UTF-8 `*const c_char`，
//! 返回的字符串必须用 [`hs_str_free`] 释放。Engine/Session 非线程安全，
//! 每个输入框一个 Session，单线程操作。
//!
//! hs_engine_load 通过二进制魔数自动识别字典类型：
//! - ZMD1 (0x31444D5A) → 形码 Table 引擎
//! - ZPY1 (0x3159505A) → 音码 Script 引擎

use crate::engine::{Engine, FeedResult, SchemaKind, Session};
use crate::core::{CandidateKey, CandidateSource, CoreRuntime, EngineStore, EventDisposition, InputEvent};
use crate::pinyin::PinyinDict;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::Arc;

pub const HS_RUNTIME_ABI_VERSION: u32 = 1;

#[repr(C)]
pub struct HsTextView {
    pub ptr: *const u8,
    pub len: u32,
}

#[repr(C)]
pub struct HsCandidateView {
    pub source: u32,
    pub ordinal: u32,
    pub word: HsTextView,
    pub annotation: HsTextView,
    pub label: HsTextView,
}

#[repr(C)]
pub struct HsRuntimeResult {
    pub disposition: u32,
    pub composition: u32,
    pub committed: HsTextView,
    pub pending: HsTextView,
    pub candidates: *const HsCandidateView,
    pub candidate_count: u32,
    pub page_index: u32,
    pub page_size: u32,
    pub total_candidates: u32,
    pub selected_source: u32,
    pub selected_ordinal: u32,
    pub has_previous: u8,
    pub has_next: u8,
    pub ascii_mode: u8,
    pub full_shape: u8,
    pub composing: u8,
    pub error_code: u32,
}

#[repr(C)]
pub struct HsRuntimeEvent {
    pub opcode: u32,
    pub value: i64,
    pub source: u32,
    pub ordinal: u32,
}

struct RuntimeResultOwner {
    result: HsRuntimeResult,
    _committed: CString,
    _pending: CString,
    candidates: Vec<HsCandidateView>,
    candidate_text: Vec<(Box<CString>, Box<CString>, Box<CString>)>,
}

struct RuntimeHandle {
    runtime: CoreRuntime,
}

fn text_view(value: &CString) -> HsTextView {
    HsTextView { ptr: value.as_ptr() as *const u8, len: value.as_bytes().len() as u32 }
}

fn runtime_result(result: crate::core::CommandResult) -> *mut c_void {
    let snapshot = result.snapshot;
    let committed = CString::new(result.committed.unwrap_or_default()).unwrap_or_default();
    let pending = CString::new(snapshot.pending).unwrap_or_default();
    let mut owner = Box::new(RuntimeResultOwner {
        result: HsRuntimeResult {
            disposition: match result.disposition { EventDisposition::Consumed => 1, EventDisposition::PassedThrough => 0 },
            composition: match result.composition { crate::core::CompositionAction::Keep => 0, crate::core::CompositionAction::Update => 1, crate::core::CompositionAction::End => 2 },
            committed: text_view(&committed), pending: text_view(&pending), candidates: ptr::null(),
            candidate_count: 0, page_index: snapshot.candidates.page_index as u32,
            page_size: snapshot.candidates.page_size as u32, total_candidates: snapshot.candidates.total as u32,
            selected_source: snapshot.candidates.selected.map(|key| key.source as u32).unwrap_or(0),
            selected_ordinal: snapshot.candidates.selected.map(|key| key.ordinal).unwrap_or(0),
            has_previous: snapshot.candidates.has_previous as u8, has_next: snapshot.candidates.has_next as u8,
            ascii_mode: snapshot.status.ascii_mode as u8, full_shape: snapshot.status.full_shape as u8,
            composing: snapshot.status.composing as u8,
            error_code: if result.error.is_some() { 1 } else { 0 },
        },
        _committed: committed, _pending: pending, candidates: Vec::new(), candidate_text: Vec::new(),
    });
    for candidate in snapshot.candidates.items {
        let word = Box::new(CString::new(candidate.word).unwrap_or_default());
        let annotation = Box::new(CString::new(candidate.annotation).unwrap_or_default());
        let label = Box::new(CString::new(candidate.label).unwrap_or_default());
        owner.candidate_text.push((word, annotation, label));
        let (word, annotation, label) = owner.candidate_text.last().unwrap();
        owner.candidates.push(HsCandidateView {
            source: candidate.key.source as u32, ordinal: candidate.key.ordinal,
            word: text_view(word), annotation: text_view(annotation), label: text_view(label),
        });
    }
    owner.result.candidates = owner.candidates.as_ptr();
    owner.result.candidate_count = owner.candidates.len() as u32;
    Box::into_raw(owner) as *mut c_void
}

fn runtime_event(event: &HsRuntimeEvent) -> Option<InputEvent> {
    Some(match event.opcode {
        0 => InputEvent::Text(char::from_u32(event.value as u32)?),
        1 => InputEvent::Backspace,
        2 => InputEvent::Delete,
        3 => InputEvent::Escape,
        4 => InputEvent::Space,
        5 => InputEvent::Enter,
        6 => InputEvent::Select(CandidateKey { source: match event.source { 1 => CandidateSource::Table, 2 => CandidateSource::ScriptExact, 3 => CandidateSource::ScriptSentence, 4 => CandidateSource::ScriptPrefix, 5 => CandidateSource::Reverse, _ => return None }, ordinal: event.ordinal }),
        7 => InputEvent::MoveSelection(event.value as i32),
        8 => InputEvent::Page(event.value as i32),
        9 => InputEvent::ToggleAscii,
        10 => InputEvent::ToggleFullShape,
        12 => InputEvent::Reset,
        _ => return None,
    })
}

#[no_mangle]
pub extern "C" fn hs_runtime_abi_version() -> u32 { HS_RUNTIME_ABI_VERSION }

#[no_mangle]
pub extern "C" fn hs_runtime_new_schema(schema_path: *const c_char) -> *mut c_void {
    let Some(path) = cstr(schema_path) else { return ptr::null_mut() };
    let Ok(engine) = Engine::from_schema_file(std::path::Path::new(path)) else { return ptr::null_mut() };
    let mut store = EngineStore::new();
    store.insert("default", engine);
    let Ok(runtime) = CoreRuntime::new(Arc::new(store), "default") else { return ptr::null_mut() };
    Box::into_raw(Box::new(RuntimeHandle { runtime })) as *mut c_void
}

#[no_mangle]
pub extern "C" fn hs_runtime_free(runtime: *mut c_void) {
    if !runtime.is_null() { drop(unsafe { Box::from_raw(runtime as *mut RuntimeHandle) }); }
}

#[no_mangle]
pub extern "C" fn hs_runtime_event(runtime: *mut c_void, event: *const HsRuntimeEvent) -> *mut c_void {
    if runtime.is_null() || event.is_null() { return ptr::null_mut(); }
    let Some(event) = runtime_event(unsafe { &*event }) else { return ptr::null_mut() };
    let handle = unsafe { &mut *(runtime as *mut RuntimeHandle) };
    runtime_result(handle.runtime.dispatch(event))
}

#[no_mangle]
pub extern "C" fn hs_runtime_result_view(result: *const c_void) -> *const HsRuntimeResult {
    if result.is_null() { ptr::null() } else { unsafe { &(*(result as *const RuntimeResultOwner)).result } }
}

#[no_mangle]
pub extern "C" fn hs_runtime_result_free(result: *mut c_void) {
    if !result.is_null() { drop(unsafe { Box::from_raw(result as *mut RuntimeResultOwner) }); }
}

#[no_mangle]
pub extern "C" fn hs_runtime_user_dict_save(runtime: *mut c_void, path: *const c_char) -> i32 {
    if runtime.is_null() { return 0; }
    let Some(path) = cstr(path) else { return 0 };
    let handle = unsafe { &*(runtime as *const RuntimeHandle) };
    handle.runtime.save_user_dict_to(std::path::Path::new(path)).map(|_| 1).unwrap_or(0)
}

/// 从二进制码表文件加载引擎。自动识别 ZMD1(形码) / ZPY1(音码)。失败返回 NULL。
#[no_mangle]
pub extern "C" fn hs_engine_load(path: *const c_char) -> *mut c_void {
    let path = match cstr(path) {
        Some(p) => p,
        None => return ptr::null_mut(),
    };
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return ptr::null_mut(),
    };
    if data.len() < 4 {
        return ptr::null_mut();
    }
    let magic = u32::from_le_bytes(data[..4].try_into().unwrap());

    let engine = match magic {
        0x31444D5A => {
            // ZMD1 — 形码
            let dict = match crate::dict::Dict::load(&data) {
                Ok(d) => d,
                Err(_) => return ptr::null_mut(),
            };
            Engine::new(SchemaKind::Table {
                dict,
                max_code_len: 4,
                auto_select: true,
                auto_select_pattern: Some("^[a-z]{4}$".into()),
            })
        }
        0x3159505A => {
            // ZPY1 — 音码（全拼/双拼）
            let dict = match PinyinDict::load(&data) {
                Ok(d) => d,
                Err(_) => return ptr::null_mut(),
            };
            Engine::new(SchemaKind::Script { dict })
        }
        _ => return ptr::null_mut(),
    };

    Box::into_raw(Box::new(engine)) as *mut c_void
}

#[no_mangle]
pub extern "C" fn hs_engine_free(eng: *mut c_void) {
    if !eng.is_null() {
        drop(unsafe { Box::from_raw(eng as *mut Engine) });
    }
}

/// 从 schema.yaml 加载引擎。失败返回 NULL。
#[no_mangle]
pub extern "C" fn hs_engine_load_schema(schema_path: *const c_char) -> *mut c_void {
    let path = match cstr(schema_path) {
        Some(p) => p,
        None => return ptr::null_mut(),
    };
    match Engine::from_schema_file(std::path::Path::new(path)) {
        Ok(engine) => Box::into_raw(Box::new(engine)) as *mut c_void,
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn hs_session_new(eng: *mut c_void) -> *mut c_void {
    if eng.is_null() {
        return ptr::null_mut();
    }
    let engine = unsafe { &*(eng as *const Engine) };
    Box::into_raw(Box::new(engine.session())) as *mut c_void
}

#[no_mangle]
pub extern "C" fn hs_session_free(sess: *mut c_void) {
    if !sess.is_null() {
        drop(unsafe { Box::from_raw(sess as *mut Session<'_>) });
    }
}

fn sess_mut<'a>(p: *mut c_void) -> Option<&'a mut Session<'a>> {
    if p.is_null() {
        None
    } else {
        Some(unsafe { &mut *(p as *mut Session<'a>) })
    }
}

fn cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

fn ret_string(s: String) -> *mut c_char {
    CString::new(s)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// feed 结果码: 0=Rejected, 1=Waiting, 2=Committed（文本经 out_committed 返回）。
#[no_mangle]
pub extern "C" fn hs_feed(
    sess: *mut c_void,
    ch: c_char,
    out_committed: *mut *mut c_char,
) -> i32 {
    let Some(s) = sess_mut(sess) else { return 0 };
    if !out_committed.is_null() {
        unsafe { *out_committed = ptr::null_mut() };
    }
    match s.feed(ch as u8 as char) {
        FeedResult::Rejected => 0,
        FeedResult::Waiting => 1,
        FeedResult::Committed(w) => {
            if !out_committed.is_null() {
                unsafe { *out_committed = ret_string(w) };
            }
            2
        }
    }
}

/// 序号选词（1-based）。成功返回上屏文本，失败返回 NULL。
#[no_mangle]
pub extern "C" fn hs_select(sess: *mut c_void, idx: i32) -> *mut c_char {
    let Some(s) = sess_mut(sess) else {
        return ptr::null_mut();
    };
    match s.select(idx as usize) {
        Some(w) => ret_string(w),
        None => ptr::null_mut(),
    }
}

/// 空格首选。无候选返回 NULL。
#[no_mangle]
pub extern "C" fn hs_select_first(sess: *mut c_void) -> *mut c_char {
    let Some(s) = sess_mut(sess) else {
        return ptr::null_mut();
    };
    match s.select_first() {
        Some(w) => ret_string(w),
        None => ptr::null_mut(),
    }
}

/// 退格。返回 1=成功 0=缓冲已空。
#[no_mangle]
pub extern "C" fn hs_backspace(sess: *mut c_void) -> i32 {
    sess_mut(sess)
        .map(|s| if s.backspace() { 1 } else { 0 })
        .unwrap_or(0)
}

/// 清空缓冲（ESC）。
#[no_mangle]
pub extern "C" fn hs_clear(sess: *mut c_void) {
    if let Some(s) = sess_mut(sess) {
        s.clear();
    }
}

/// 当前缓冲编码（预编辑显示）。调用方负责 hs_str_free。
#[no_mangle]
pub extern "C" fn hs_pending(sess: *mut c_void) -> *mut c_char {
    let Some(s) = sess_mut(sess) else {
        return ptr::null_mut();
    };
    ret_string(s.pending().to_string())
}

/// 候选列表，格式: "字词1\x01编码1\x02字词2\x01编码2\x02…"
/// limit<=0 表示不限（最多 9）。调用方负责 hs_str_free。
#[no_mangle]
pub extern "C" fn hs_candidates(sess: *mut c_void, limit: i32) -> *mut c_char {
    let Some(s) = sess_mut(sess) else {
        return ptr::null_mut();
    };
    let lim = if limit <= 0 { 9 } else { limit as usize };
    let mut out = String::new();
    for (i, c) in s.candidates(lim).iter().enumerate() {
        if i > 0 {
            out.push('\u{2}');
        }
        out.push_str(&c.word);
        out.push('\u{1}');
        out.push_str(&c.code);
    }
    ret_string(out)
}

#[no_mangle]
pub extern "C" fn hs_candidates_page(sess: *mut c_void, offset: i32, limit: i32) -> *mut c_char {
    let Some(s) = sess_mut(sess) else { return ptr::null_mut(); };
    let offset = offset.max(0) as usize;
    let limit = if limit <= 0 { 9 } else { limit as usize };
    let candidates = s.candidates(offset.saturating_add(limit));
    let mut out = String::new();
    for (i, c) in candidates.iter().skip(offset).take(limit).enumerate() {
        if i > 0 { out.push('\u{2}'); }
        out.push_str(&c.word);
        out.push('\u{1}');
        out.push_str(&c.code);
    }
    ret_string(out)
}

/// 释放本库返回的字符串。
#[no_mangle]
pub extern "C" fn hs_str_free(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { CString::from_raw(p) });
    }
}

/// 查询西文模式状态。返回 1=西文, 0=中文。
#[no_mangle]
pub extern "C" fn hs_ascii_mode(sess: *mut c_void) -> i32 {
    sess_mut(sess).map(|s| if s.ascii_mode { 1 } else { 0 }).unwrap_or(0)
}

/// 设置西文模式。ascii=1 切换为西文，0 切换为中文。
#[no_mangle]
pub extern "C" fn hs_set_ascii_mode(sess: *mut c_void, ascii: i32) {
    if let Some(s) = sess_mut(sess) {
        s.ascii_mode = ascii != 0;
    }
}

/// 持久化用户词典。返回 1=成功, 0=失败。
#[no_mangle]
pub extern "C" fn hs_user_dict_save(eng: *mut c_void, path: *const c_char) -> i32 {
    if eng.is_null() { return 0; }
    let engine = unsafe { &*(eng as *const Engine) };
    let path = match cstr(path) { Some(p) => p, None => return 0 };
    match engine.user_dict.borrow().as_ref() {
        Some(ud) => {
            ud.save(std::path::Path::new(path)).map(|_| 1).unwrap_or(0)
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::encode_code;
    use std::ffi::CString;

    fn tmp_bin_table() -> std::path::PathBuf {
        use crate::dict::Dict;
        let entries = vec![
            (encode_code("j").unwrap(), "中".into()),
            (encode_code("jivv").unwrap(), "中".into()),
            (encode_code("aa").unwrap(), "一下".into()),
        ];
        let dict = Dict::from_entries(entries);
        let mut buf = Vec::new();
        dict.save(&mut buf).unwrap();
        let path = std::env::temp_dir().join(format!("hs_ffi_test_{}.bin", std::process::id()));
        std::fs::write(&path, &buf).unwrap();
        path
    }

    fn cstring(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn ffi_end_to_end_table() {
        let path = tmp_bin_table();
        let cpath = cstring(path.to_str().unwrap());
        let eng = hs_engine_load(cpath.as_ptr());
        assert!(!eng.is_null(), "engine 加载失败");
        let sess = hs_session_new(eng);
        assert!(!sess.is_null());

        // j-i-v-v → 自动上屏「中」
        let mut out: *mut c_char = ptr::null_mut();
        for c in [b'j', b'i', b'v'] {
            assert_eq!(hs_feed(sess, c as c_char, &mut out), 1);
        }
        assert_eq!(hs_feed(sess, b'v' as c_char, &mut out), 2);
        let committed = unsafe { CStr::from_ptr(out).to_str().unwrap().to_string() };
        assert_eq!(committed, "中");
        hs_str_free(out);

        // a + a → 空格首选上屏「一下」
        assert_eq!(hs_feed(sess, b'a' as c_char, &mut out), 1);
        assert_eq!(hs_feed(sess, b'a' as c_char, &mut out), 1);
        let cands = hs_candidates(sess, 9);
        let cs = unsafe { CStr::from_ptr(cands).to_str().unwrap() };
        assert!(cs.starts_with("一下\u{1}aa"), "候选串异常: {cs:?}");
        hs_str_free(cands);
        let w = hs_select_first(sess);
        assert_eq!(unsafe { CStr::from_ptr(w).to_str().unwrap() }, "一下");
        hs_str_free(w);

        hs_session_free(sess);
        hs_engine_free(eng);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn ffi_null_safety() {
        let mut out: *mut c_char = ptr::null_mut();
        assert_eq!(hs_feed(ptr::null_mut(), b'a' as c_char, &mut out), 0);
        assert!(hs_select(ptr::null_mut(), 1).is_null());
        assert!(hs_select_first(ptr::null_mut()).is_null());
        assert!(hs_pending(ptr::null_mut()).is_null());
        assert_eq!(hs_backspace(ptr::null_mut()), 0);
        hs_str_free(ptr::null_mut());
        hs_session_free(ptr::null_mut());
        hs_engine_free(ptr::null_mut());
    }

    #[test]
    fn owned_runtime_abi_returns_bounded_views() {
        let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("schemas")
            .join("zhengma66.schema.yaml");
        let cpath = cstring(schema.to_str().unwrap());
        assert_eq!(hs_runtime_abi_version(), HS_RUNTIME_ABI_VERSION);
        let runtime = hs_runtime_new_schema(cpath.as_ptr());
        assert!(!runtime.is_null());
        let event = HsRuntimeEvent { opcode: 0, value: 'a' as i64, source: 0, ordinal: 0 };
        let result = hs_runtime_event(runtime, &event);
        assert!(!result.is_null());
        let view = hs_runtime_result_view(result);
        assert!(!view.is_null());
        let view = unsafe { &*view };
        assert_eq!(view.disposition, 1);
        assert_eq!(view.pending.len, 1);
        assert!(view.candidate_count > 0);
        assert!(!view.candidates.is_null());
        let first = unsafe { &*view.candidates };
        assert!(first.word.len > 0);
        hs_runtime_result_free(result);
        assert!(hs_runtime_event(ptr::null_mut(), &event).is_null());
        let invalid = HsRuntimeEvent { opcode: 999, value: 0, source: 0, ordinal: 0 };
        assert!(hs_runtime_event(runtime, &invalid).is_null());
        hs_runtime_free(runtime);
        assert!(hs_runtime_result_view(ptr::null()).is_null());
    }
}
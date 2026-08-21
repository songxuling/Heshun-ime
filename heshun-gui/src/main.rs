//! heshun-gui — 通用中文输入法引擎跨平台 GUI demo
//!
//! 基于 egui，支持：郑码 / 全拼 / 自然码双拼 三种方案实时切换。
//! 键盘输入 → 引擎 → 候选面板 → 上屏。

use eframe::egui;
use heshun::engine::{Engine, FeedResult};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("heshun 输入法引擎 demo"),
        ..Default::default()
    };
    eframe::run_native("heshun-gui", options, Box::new(|cc| {
        configure_fonts(&cc.egui_ctx);
        Ok(Box::new(App::new()))
    }))
}

/// egui 默认内置字体不包含 CJK 字形；将 Windows 的微软雅黑作为优先回退字体。
/// 仅打包字体会使 demo 体积额外增加约 19 MB，因此开发版直接读取系统字体。
fn configure_fonts(ctx: &egui::Context) {
    const FONT_PATH: &str = "C:/Windows/Fonts/msyh.ttc";
    let Ok(bytes) = std::fs::read(FONT_PATH) else {
        eprintln!("未找到中文字体 {FONT_PATH}；中文可能显示为方框。");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "microsoft_yahei".to_owned(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "microsoft_yahei".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// 支持的输入方案
const SCHEMES: &[(&str, &str)] = &[
    ("zhengma66", "郑码6.6"),
    ("pinyin_full", "全拼"),
    ("double_pinyin_zrm", "自然码双拼"),
];

struct App {
    engine: Engine,
    buf: String,
    sentence_cands: Vec<heshun::composer::SentenceCandidate>,
    output: String,
    // 可见候选（词 + 注释）；注释通常是编码，郑码反查时为对应郑码。
    candidates: Vec<heshun::engine::Candidate>,
    // 编辑器光标位置（字符索引，而非 UTF-8 字节索引）。
    cursor_char: usize,
    // 候选翻页偏移，页大小固定为 9。
    candidate_page: usize,
    last_error: Option<String>,
    // 唯一的键盘输入目标。持续申请焦点，避免候选窗/工具栏点击后失焦。
    editor_id: egui::Id,
    scheme_idx: usize,
    ascii_mode: bool,
}

impl App {
    fn schema_dir() -> Result<std::path::PathBuf, String> {
        let exe_dir = std::env::current_exe()
            .map_err(|e| format!("无法定位程序目录: {e}"))?
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "程序路径没有父目录".to_owned())?;
        // 发布包：exe 同级 schemas；开发 Monorepo：heshun/schemas。
        for dir in std::iter::once(exe_dir.clone())
            .chain(exe_dir.ancestors().skip(1).map(|p| p.to_path_buf())) {
            let standalone = dir.join("schemas");
            if standalone.is_dir() { return Ok(standalone); }
            let workspace = dir.join("heshun").join("schemas");
            if workspace.is_dir() { return Ok(workspace); }
        }
        Err("未找到 schemas 资源目录；请将 schemas 放在程序目录，或放在 heshun/schemas".to_owned())
    }

    fn load_scheme(&mut self, idx: usize) -> Result<(), String> {
        let (id, _) = SCHEMES[idx];
        let schema_dir = Self::schema_dir()?;
        let schema = schema_dir.join(format!("{id}.schema.yaml"));
        self.engine = Engine::from_schema_file(&schema)?;
        self.buf.clear();
        self.sentence_cands.clear();
        self.candidates.clear();
        self.candidate_page = 0;
        self.scheme_idx = idx;
        self.last_error = None;
        Ok(())
    }

    fn switch_scheme(&mut self, idx: usize) {
        if idx == self.scheme_idx { return; }
        if let Err(error) = self.engine.save_user_dict() {
            self.last_error = Some(error);
            return;
        }
        if let Err(error) = self.load_scheme(idx) {
            self.last_error = Some(error);
        }
    }

    fn save_user_dict(&mut self) {
        if let Err(error) = self.engine.save_user_dict() {
            self.last_error = Some(error);
        }
    }

    fn new() -> Self {
        let editor_id = egui::Id::new("ime_output_editor");
        let fallback = Engine::new(heshun::engine::SchemaKind::Table {
            dict: heshun::dict::Dict::from_entries(Vec::new()),
            max_code_len: 4,
            auto_select: false,
            auto_select_pattern: None,
        });
        let mut app = App {
            engine: fallback, buf: String::new(), sentence_cands: Vec::new(),
            output: String::new(), candidates: Vec::new(),
            cursor_char: 0, candidate_page: 0, last_error: None,
            editor_id, scheme_idx: 0, ascii_mode: false,
        };
        if let Err(error) = app.load_scheme(0) {
            app.last_error = Some(error);
        }
        app
    }

    fn insert_committed(&mut self, text: &str) {
        let byte = self.output.char_indices().nth(self.cursor_char)
            .map(|(i, _)| i).unwrap_or(self.output.len());
        self.output.insert_str(byte, text);
        self.cursor_char += text.chars().count();
    }

    fn delete_before_cursor(&mut self) {
        if self.cursor_char == 0 { return; }
        let start = self.output.char_indices().nth(self.cursor_char - 1).map(|(i, _)| i).unwrap_or(0);
        let end = self.output.char_indices().nth(self.cursor_char).map(|(i, _)| i).unwrap_or(self.output.len());
        self.output.replace_range(start..end, "");
        self.cursor_char -= 1;
    }

    fn refresh_candidates(&mut self) {
        self.candidate_page = 0;
        if self.buf.is_empty() {
            self.candidates.clear();
            return;
        }
        let mut session = self.engine.session();
        session.restore_state(self.buf.clone(), self.sentence_cands.clone());
        session.ascii_mode = self.ascii_mode;
        self.candidates = session.candidates(0);
    }

    fn handle_key(&mut self, ch: char) {
        // 构建临时 session
        let mut sess = self.engine.session();
        sess.restore_state(std::mem::take(&mut self.buf), std::mem::take(&mut self.sentence_cands));
        sess.ascii_mode = self.ascii_mode;

        let mut delete_output = false;
        let committed = match ch {
            '\u{8}' | '\u{7f}' => {
                delete_output = !sess.backspace();
                None
            }
            '\u{1b}' => { sess.clear(); None }
            ' ' => self.candidates.first().and_then(|c| sess.select_word(&c.word)),
            '1'..='9' => {
                let index = self.candidate_page * 9 + (ch as usize - '0' as usize - 1);
                self.candidates.get(index).and_then(|c| sess.select_word(&c.word))
            }
            _ => match sess.feed(ch) {
                FeedResult::Committed(text) => Some(text),
                _ => None,
            },
        };

        // 保存 session 状态
        let (b, sc) = sess.take_state();
        self.buf = b;
        self.sentence_cands = sc;
        self.ascii_mode = sess.ascii_mode;
        drop(sess);
        if delete_output { self.delete_before_cursor(); }
        if let Some(text) = committed { self.insert_committed(&text); }

        self.refresh_candidates();
    }

    fn select_visible_candidate(&mut self, index: usize) {
        let Some(word) = self.candidates.get(index).map(|c| c.word.clone()) else { return };
        let mut session = self.engine.session();
        session.restore_state(std::mem::take(&mut self.buf), std::mem::take(&mut self.sentence_cands));
        session.ascii_mode = self.ascii_mode;
        let committed = session.select_word(&word);
        let (buf, sentence_cands) = session.take_state();
        self.buf = buf;
        self.sentence_cands = sentence_cands;
        self.ascii_mode = session.ascii_mode;
        drop(session);
        if let Some(text) = committed { self.insert_committed(&text); }
        self.refresh_candidates();
    }

    /// 将 TextEdit 的插入光标同步到本程序维护的字符位置。
    fn sync_text_cursor(&self, ctx: &egui::Context) {
        let mut state = egui::text_edit::TextEditState::load(ctx, self.editor_id).unwrap_or_default();
        let cursor = egui::text::CCursor::new(self.cursor_char.min(self.output.chars().count()));
        state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::one(cursor)));
        state.store(ctx, self.editor_id);
    }

}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_user_dict();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── 1. 处理键盘事件 ──────────────────────────
        // Text 输入与 Backspace/Escape 是不同的 egui event：后两者是 Event::Key，
        // 因此不能只读取 Event::Text，否则编辑区会收到按键、引擎却收不到。
        let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());
        let mut input_chars = Vec::new();
        let mut page_delta: i32 = 0;
        for event in &events {
            match event {
                egui::Event::Text(text) => input_chars.extend(text.chars()),
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                    input_chars.push('\u{8}');
                }
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    input_chars.push('\u{1b}');
                }
                egui::Event::Key { key: egui::Key::PageUp, pressed: true, .. } => page_delta = -1,
                egui::Event::Key { key: egui::Key::PageDown, pressed: true, .. } => page_delta = 1,
                _ => {}
            }
        }
        // 已交给输入引擎的事件必须从 egui 队列移除：
        // 否则 TextEdit 还会处理 Backspace，误删已经上屏的文本。
        ctx.input_mut(|i| {
            i.events.retain(|event| !matches!(event,
                egui::Event::Text(_)
                | egui::Event::Key { key: egui::Key::Backspace | egui::Key::Escape | egui::Key::PageUp | egui::Key::PageDown, pressed: true, .. }
            ));
        });

        if page_delta != 0 && !self.candidates.is_empty() {
            let last_page = self.candidates.len().saturating_sub(1) / 9;
            self.candidate_page = if page_delta < 0 {
                self.candidate_page.saturating_sub(1)
            } else {
                (self.candidate_page + 1).min(last_page)
            };
        }
        for ch in input_chars {
            self.handle_key(ch);
        }
        // 引擎上屏后同步插入点到本程序维护的位置。
        self.sync_text_cursor(ctx);
        // 输入法 demo 始终将键盘输入路由到主编辑区；候选按钮和工具栏点击不应夺走输入焦点。
        ctx.memory_mut(|mem| mem.request_focus(self.editor_id));

        // ── 2. 顶部工具栏 ──────────────────────────
        if let Some(error) = &self.last_error {
            egui::TopBottomPanel::top("error").show(ctx, |ui| {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("资源/保存错误：{error}"));
            });
        }
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("方案：");
                let (_, current_name) = SCHEMES[self.scheme_idx];
                egui::ComboBox::from_id_salt("scheme")
                    .selected_text(current_name)
                    .show_ui(ui, |ui| {
                        for (i, (_, name)) in SCHEMES.iter().enumerate() {
                            if ui.selectable_label(i == self.scheme_idx, *name).clicked() {
                                self.switch_scheme(i);
                            }
                        }
                    });
                ui.separator();
                let mode = if self.ascii_mode { "EN" } else { "中" };
                ui.label(format!("模式：{mode}"));
                if ui.button("切换").clicked() {
                    self.ascii_mode = !self.ascii_mode;
                }
            });
        });

        // ── 3. 中央区域 ────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.buf.is_empty() {
                let mode_hint = if self.ascii_mode { " [EN]" } else { "" };
                ui.label(format!("编码：{}{}", self.buf, mode_hint));
            }
            let edit = egui::TextEdit::multiline(&mut self.output)
                .id(self.editor_id)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Body)
                .show(ui);
            if let Some(range) = edit.cursor_range {
                self.cursor_char = range.primary.ccursor.index.min(self.output.chars().count());
            }

            // 候选面板：每页 9 个，翻页后数字键 1-9 对应当前页。
            if !self.candidates.is_empty() {
                let page_count = self.candidates.len().div_ceil(9);
                self.candidate_page = self.candidate_page.min(page_count.saturating_sub(1));
                let start = self.candidate_page * 9;
                let end = (start + 9).min(self.candidates.len());
                let mut clicked = None;
                egui::Window::new("candidates")
                    .title_bar(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -20.0])
                    .show(ctx, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (visible, cand) in self.candidates[start..end].iter().enumerate() {
                                let label = format!("{}. {}  [{}]", visible + 1, cand.word, cand.code);
                                if ui.button(label).clicked() { clicked = Some(start + visible); }
                            }
                        });
                        if page_count > 1 {
                            ui.horizontal(|ui| {
                                if ui.button("上一页").clicked() && self.candidate_page > 0 {
                                    self.candidate_page -= 1;
                                }
                                ui.label(format!("{}/{} 页", self.candidate_page + 1, page_count));
                                if ui.button("下一页").clicked() && self.candidate_page + 1 < page_count {
                                    self.candidate_page += 1;
                                }
                            });
                        }
                    });
                if let Some(index) = clicked { self.select_visible_candidate(index); }
            }
        });

        // ── 4. 底部状态栏 ──────────────────────────
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let (_, name) = SCHEMES[self.scheme_idx];
            ui.label(format!("{} | {} 字已上屏", name, self.output.chars().count()));
        });

        ctx.request_repaint();
    }
}
//! heshun-gui — 通用中文输入法引擎跨平台 GUI demo
//!
//! 基于 egui，支持：郑码 / 全拼 / 自然码双拼 三种方案实时切换。
//! 键盘输入 → 引擎 → 候选面板 → 上屏。

use eframe::egui;
use heshun::{CandidateKey, CoreRuntime, Engine, EngineStore, InputEvent};
use std::sync::Arc;

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
    runtime: CoreRuntime,
    output: String,
    // 编辑器光标位置（字符索引，而非 UTF-8 字节索引）。
    cursor_char: usize,
    last_error: Option<String>,
    // 唯一的键盘输入目标。持续申请焦点，避免候选窗/工具栏点击后失焦。
    editor_id: egui::Id,
    scheme_idx: usize,
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

    fn switch_scheme(&mut self, idx: usize) {
        if idx == self.scheme_idx { return; }
        if let Err(error) = self.runtime.save_user_dict() {
            self.last_error = Some(error);
            return;
        }
        let result = self.runtime.dispatch(InputEvent::SetSchema(SCHEMES[idx].0.to_owned()));
        if let Some(error) = result.error {
            self.last_error = Some(format!("方案切换失败：{error:?}"));
        } else {
            self.scheme_idx = idx;
            self.last_error = None;
        }
    }

    fn save_user_dict(&mut self) {
        if let Err(error) = self.runtime.save_user_dict() {
            self.last_error = Some(error);
        }
    }

    fn new() -> Self {
        let editor_id = egui::Id::new("ime_output_editor");
        let mut store = EngineStore::new();
        let mut load_errors = Vec::new();
        if let Ok(schema_dir) = Self::schema_dir() {
            for (id, _) in SCHEMES {
                match Engine::from_schema_file(&schema_dir.join(format!("{id}.schema.yaml"))) {
                    Ok(engine) => store.insert(*id, engine),
                    Err(error) => load_errors.push(format!("{id}: {error}")),
                }
            }
        } else {
            load_errors.push("未找到 schemas 资源目录".to_owned());
        }
        if !store.contains(SCHEMES[0].0) {
            store.insert(SCHEMES[0].0, Engine::new(heshun::engine::SchemaKind::Table {
                dict: heshun::dict::Dict::from_entries(Vec::new()),
                max_code_len: 4,
                auto_select: false,
                auto_select_pattern: None,
            }));
        }
        let runtime = CoreRuntime::new(Arc::new(store), SCHEMES[0].0)
            .expect("fallback schema must be available");
        let mut app = App {
            runtime, output: String::new(), cursor_char: 0, last_error: None,
            editor_id, scheme_idx: 0,
        };
        if !load_errors.is_empty() {
            app.last_error = Some(load_errors.join("；"));
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

    fn handle_key(&mut self, ch: char) {
        let event = match ch {
            '\u{8}' | '\u{7f}' => {
                InputEvent::Backspace
            }
            '\u{1b}' => InputEvent::Escape,
            ' ' => InputEvent::Space,
            '1'..='9' => {
                let snapshot = self.runtime.snapshot();
                let index = ch as usize - '1' as usize;
                snapshot.candidates.items.get(index)
                    .map(|candidate| InputEvent::Select(candidate.key))
                    .unwrap_or(InputEvent::Select(heshun::CandidateKey {
                        source: heshun::CandidateSource::Table,
                        ordinal: u32::MAX,
                    }))
            }
            _ => InputEvent::Text(ch),
        };
        let result = self.runtime.dispatch(event);
        if matches!(result.disposition, heshun::EventDisposition::PassedThrough)
            && matches!(ch, '\u{8}' | '\u{7f}')
        {
            self.delete_before_cursor();
        }
        if let Some(error) = result.error {
            self.last_error = Some(format!("输入处理失败：{error:?}"));
        }
        if let Some(text) = result.committed { self.insert_committed(&text); }
    }

    fn select_visible_candidate(&mut self, key: CandidateKey) {
        let result = self.runtime.dispatch(InputEvent::Select(key));
        if let Some(error) = result.error {
            self.last_error = Some(format!("候选选择失败：{error:?}"));
        }
        if let Some(text) = result.committed { self.insert_committed(&text); }
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

        if page_delta != 0 {
            self.runtime.dispatch(InputEvent::Page(page_delta));
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
                let mode = if self.runtime.snapshot().status.ascii_mode { "EN" } else { "中" };
                ui.label(format!("模式：{mode}"));
                if ui.button("切换").clicked() {
                    self.runtime.dispatch(InputEvent::ToggleAscii);
                }
            });
        });

        // ── 3. 中央区域 ────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            let snapshot = self.runtime.snapshot();
            if !snapshot.pending.is_empty() {
                let mode_hint = if snapshot.status.ascii_mode { " [EN]" } else { "" };
                ui.label(format!("编码：{}{}", snapshot.pending, mode_hint));
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
            if !snapshot.candidates.items.is_empty() {
                let page_count = snapshot.candidates.total.div_ceil(snapshot.candidates.page_size);
                let mut clicked = None;
                egui::Window::new("candidates")
                    .title_bar(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -20.0])
                    .show(ctx, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for cand in &snapshot.candidates.items {
                                let label = format!("{}. {}  [{}]", cand.label, cand.word, cand.annotation);
                                if ui.button(label).clicked() { clicked = Some(cand.key); }
                            }
                        });
                        if page_count > 1 {
                            ui.horizontal(|ui| {
                                if ui.button("上一页").clicked() && snapshot.candidates.has_previous {
                                    self.runtime.dispatch(InputEvent::Page(-1));
                                }
                                ui.label(format!("{}/{} 页", snapshot.candidates.page_index + 1, page_count));
                                if ui.button("下一页").clicked() && snapshot.candidates.has_next {
                                    self.runtime.dispatch(InputEvent::Page(1));
                                }
                            });
                        }
                    });
                if let Some(key) = clicked { self.select_visible_candidate(key); }
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
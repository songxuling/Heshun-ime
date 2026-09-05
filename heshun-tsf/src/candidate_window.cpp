#include "candidate_window.h"
#include <windowsx.h>

#include <algorithm>
#include <cmath>
#include <mutex>
#include <filesystem>
#include <fstream>

extern HINSTANCE g_module_instance;

namespace {
void CandidateTrace(const std::string& message) {
    wchar_t module_path[MAX_PATH]{};
    const DWORD length = GetModuleFileNameW(g_module_instance, module_path, ARRAYSIZE(module_path));
    if (!length || length >= ARRAYSIZE(module_path)) return;
    std::filesystem::path log_path(module_path, module_path + length);
    log_path = log_path.parent_path() / L"heshun-tsf.log";
    std::ofstream log(log_path, std::ios::app);
    if (log) log << message << '\n';
}

constexpr wchar_t kClassName[] = L"HeshunTsfCandidateWindow";
constexpr UINT_PTR kCaretTimer = 1;
constexpr UINT kCaretBlinkMs = 530;

int DpiForWindow(HWND window) {
    const UINT dpi = window ? GetDpiForWindow(window) : 96;
    return dpi ? static_cast<int>(dpi) : 96;
}

HFONT MakeFont(HWND window, int points, const std::wstring& family) {
    const int dpi = DpiForWindow(window);
    const int height = -MulDiv(points, dpi, 72);
    return CreateFontW(height, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                       DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                       CLEARTYPE_QUALITY, DEFAULT_PITCH | FF_DONTCARE, family.c_str());
}

int ReadStyleInt(const std::wstring& path, const wchar_t* key, int fallback) {
    return GetPrivateProfileIntW(L"candidate", key, fallback, path.c_str());
}

COLORREF ReadStyleColor(const std::wstring& path, const wchar_t* key, COLORREF fallback) {
    return static_cast<COLORREF>(ReadStyleInt(path, key, static_cast<int>(fallback)));
}
}

CandidateWindow::~CandidateWindow() {
    Hide();
    if (window_) DestroyWindow(window_);
}

bool CandidateWindow::EnsureWindow() {
    if (window_) return true;

    static std::once_flag registered;
    std::call_once(registered, [] {
        WNDCLASSW wc{};
        wc.lpfnWndProc = &CandidateWindow::WindowProc;
        wc.hInstance = GetModuleHandleW(nullptr);
        wc.hCursor = LoadCursorW(nullptr, IDC_ARROW);
        wc.hbrBackground = nullptr;
        wc.lpszClassName = kClassName;
        RegisterClassW(&wc);
    });

    window_ = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                              kClassName, L"heshun candidates", WS_POPUP,
                              0, 0, 480, 80, nullptr, nullptr,
                              GetModuleHandleW(nullptr), this);
    return window_ != nullptr;
}

void CandidateWindow::Show(std::wstring pending, std::vector<std::wstring> candidates, std::vector<CandidateKey> keys, unsigned int page_index, unsigned int page_size, unsigned int total, unsigned int cursor) {
    ReloadStyle();
    const bool content_changed = pending_ != pending || candidates_ != candidates || keys_ != keys;
    pending_ = std::move(pending);
    candidates_ = std::move(candidates);
    keys_ = std::move(keys);
    page_index_ = page_index;
    page_size_ = page_size ? page_size : 9;
    total_candidates_ = total;
    cursor_ = std::min(cursor, static_cast<unsigned int>(pending_.size()));
    if (content_changed) {
        selected_index_ = 0;
        keyboard_selection_ = false;
    }
    if (pending_.empty() || !EnsureWindow()) {
        Hide();
        return;
    }

    // Resolve the anchor caret before sizing/placing the popup.  rcCaret from
    // GetGUIThreadInfo is client-relative to hwndCaret; the previous code
    // sized from the popup's old monitor and could also use an unconverted
    // caret rectangle, which breaks on per-monitor DPI and secondary displays.
    GUITHREADINFO info{sizeof(info)};
    HWND foreground = GetForegroundWindow();
    POINT anchor{};
    HWND anchor_window = nullptr;
    RECT foreground_rect{};
    const bool has_foreground_rect = foreground && GetWindowRect(foreground, &foreground_rect);
    POINT gui_caret{};
    bool has_gui_caret = false;
    const bool has_tsf_anchor = has_anchor_rect_ &&
        anchor_rect_.right >= anchor_rect_.left && anchor_rect_.bottom >= anchor_rect_.top;
    RECT corrected_tsf_rect = anchor_rect_;
    bool corrected_tsf_anchor = false;
    if (has_tsf_anchor && has_foreground_rect) {
        GUITHREADINFO caret_info{sizeof(caret_info)};
        const bool has_raw_gui_caret = GetGUIThreadInfo(
            GetWindowThreadProcessId(foreground, nullptr), &caret_info) &&
            caret_info.hwndCaret && caret_info.rcCaret.right >= caret_info.rcCaret.left &&
            caret_info.rcCaret.bottom >= caret_info.rcCaret.top;
        if (has_raw_gui_caret) {
            gui_caret = POINT{caret_info.rcCaret.left, caret_info.rcCaret.bottom};
            has_gui_caret = true;
            if (caret_info.hwndCaret != foreground) {
                ClientToScreen(caret_info.hwndCaret, &gui_caret);
                gui_caret.x -= foreground_rect.left;
                gui_caret.y -= foreground_rect.top;
            }
        } else {
            POINT caret_pos{};
            if (GetCaretPos(&caret_pos)) {
                gui_caret = caret_pos;
                has_gui_caret = true;
                CandidateTrace("CandidateWindow: using thread GetCaretPos fallback");
            }
        }
        corrected_tsf_rect = CorrectCandidateAnchorRect(
            corrected_tsf_rect, foreground_rect, has_gui_caret, gui_caret,
            &corrected_tsf_anchor);
        if (corrected_tsf_anchor) {
            CandidateTrace("CandidateWindow: corrected out-of-window TSF rect");
        }
    }
    const bool has_tsf_point = has_tsf_anchor;
    const POINT tsf_anchor{corrected_tsf_rect.left, corrected_tsf_rect.bottom};
    if (!has_tsf_anchor) {
        has_gui_caret = foreground &&
            GetGUIThreadInfo(GetWindowThreadProcessId(foreground, nullptr), &info) &&
            info.hwndCaret && info.rcCaret.right >= info.rcCaret.left &&
            info.rcCaret.bottom >= info.rcCaret.top;
        if (has_gui_caret) {
            anchor = POINT{info.rcCaret.left, info.rcCaret.bottom};
            anchor_window = info.hwndCaret;
            if (!ClientToScreen(anchor_window, &anchor)) {
                anchor_window = nullptr;
                has_gui_caret = false;
            }
        }
    }
    const POINT fallback_anchor{foreground ? 0 : 24, foreground ? 0 : 24};
    if (!has_tsf_anchor && !has_gui_caret) {
        CandidateTrace("CandidateWindow: no reliable caret anchor; defer show");
        Hide();
        return;
    }
    anchor = ResolveCandidateAnchorPoint(has_tsf_anchor, tsf_anchor,
                                         has_gui_caret, anchor, fallback_anchor);
    const bool has_caret = has_tsf_anchor || has_gui_caret;
    if (!has_tsf_anchor && !anchor_window) {
        anchor_window = foreground;
        if (anchor_window) ClientToScreen(anchor_window, &anchor);
    }

    const int dpi = DpiForWindow(anchor_window ? anchor_window : window_);
    const int scale = dpi;
    const int row = MulDiv(style_.row_height, scale, 96);
    const int padding = MulDiv(style_.padding, scale, 96);
    const int rows = static_cast<int>(std::min<size_t>(9, candidates_.size()));
    HFONT measure_font = MakeFont(window_, style_.font_size, style_.font_family);
    HDC measure_dc = GetDC(window_);
    HGDIOBJ old_measure_font = nullptr;
    if (measure_dc && measure_font) old_measure_font = SelectObject(measure_dc, measure_font);
    SIZE max_text_size{};
    if (measure_dc) {
        for (const auto& candidate : candidates_) {
            const size_t annotation_start = candidate.find(L"  [");
            const std::wstring word = annotation_start == std::wstring::npos
                ? candidate : candidate.substr(0, annotation_start);
            const std::wstring annotation = annotation_start == std::wstring::npos
                ? L"" : candidate.substr(annotation_start);
            SIZE word_size{};
            SIZE annotation_size{};
            const bool word_measured = GetTextExtentPoint32W(
                measure_dc, word.c_str(), static_cast<int>(word.size()), &word_size);
            const bool annotation_measured = annotation.empty() ||
                GetTextExtentPoint32W(measure_dc, annotation.c_str(),
                                      static_cast<int>(annotation.size()), &annotation_size);
            if (word_measured && annotation_measured) {
                const int rendered_width = static_cast<int>(word_size.cx) +
                    (annotation.empty() ? 0 : MulDiv(8, scale, 96) + static_cast<int>(annotation_size.cx));
                max_text_size.cx = static_cast<LONG>(std::max(static_cast<int>(max_text_size.cx), rendered_width));
            }
        }
        if (old_measure_font) SelectObject(measure_dc, old_measure_font);
        ReleaseDC(window_, measure_dc);
    }
    if (measure_font) DeleteObject(measure_font);
    const int badge_size = std::max(18, row - MulDiv(10, scale, 96));
    const int width_padding = padding * 2;
    // The renderer offsets the badge by 2px inside the content area. Include
    // that offset so a long annotation is not clipped by the right padding.
    const int measured_content = max_text_size.cx + width_padding + MulDiv(2, scale, 96);
    const int client_width = DynamicCandidateClientWidth(
        measured_content, badge_size, MulDiv(10, scale, 96),
        MulDiv(style_.min_width, scale, 96), MulDiv(style_.max_width, scale, 96));
    const int client_height = CandidateWindowContentHeight(static_cast<size_t>(rows), row, padding);
    RECT frame{0, 0, client_width, client_height};
    AdjustWindowRectExForDpi(&frame, WS_POPUP, FALSE, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST, dpi);
    const int width = frame.right - frame.left;
    const int height = frame.bottom - frame.top;

    if (style_.corner_radius > 0) {
        const int radius = MulDiv(style_.corner_radius, scale, 96);
        HRGN region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2);
        if (region) SetWindowRgn(window_, region, FALSE);
    }

    HMONITOR monitor = MonitorFromPoint(anchor, MONITOR_DEFAULTTONEAREST);
    MONITORINFO monitor_info{sizeof(monitor_info)};
    GetMonitorInfoW(monitor, &monitor_info);
    const RECT work = monitor_info.rcWork;
    const int x = std::clamp(anchor.x, work.left, std::max(work.left, work.right - width));
    const int y = CandidateWindowTopForAnchor(corrected_tsf_rect.top, corrected_tsf_rect.bottom,
                                               height, work.top, work.bottom, has_caret ? 4 : 0);
    CandidateTrace("CandidateWindow: anchor=" + std::to_string(anchor.x) + "," +
                   std::to_string(anchor.y) + " tsf=" + std::to_string(has_tsf_anchor) +
                   " gui=" + std::to_string(has_gui_caret) + " final=" + std::to_string(x) +
                   "," + std::to_string(y));
    SetWindowPos(window_, HWND_TOPMOST, x, y, width, height,
                 SWP_NOACTIVATE | SWP_SHOWWINDOW);
    InvalidateRect(window_, nullptr, TRUE);
    UpdateWindow(window_);
    caret_visible_ = true;
    SetTimer(window_, kCaretTimer, kCaretBlinkMs, nullptr);
}

void CandidateWindow::SetCandidateClickHandler(std::function<void(CandidateKey)> handler) {
    candidate_click_handler_ = std::move(handler);
}

void CandidateWindow::SetPageChangeHandler(std::function<void(int)> handler) {
    page_change_handler_ = std::move(handler);
}

void CandidateWindow::MoveSelection(int direction) {
    const size_t count = std::min<size_t>(9, candidates_.size());
    if (!count) return;
    const int next = static_cast<int>(selected_index_) + direction;
    selected_index_ = static_cast<size_t>((next % static_cast<int>(count) + static_cast<int>(count)) % static_cast<int>(count));
    InvalidateRect(window_, nullptr, FALSE);
}

void CandidateWindow::SetSelection(size_t index) {
    const size_t count = std::min<size_t>(9, candidates_.size());
    if (!count) return;
    selected_index_ = std::min(index, count - 1);
    InvalidateRect(window_, nullptr, FALSE);
}

void CandidateWindow::UseKeyboardSelection() {
    keyboard_selection_ = true;
}

void CandidateWindow::Hide() {
    if (window_) {
        KillTimer(window_, kCaretTimer);
        ShowWindow(window_, SW_HIDE);
    }
    pending_.clear();
    candidates_.clear();
    keys_.clear();
    page_index_ = 0;
    total_candidates_ = 0;
    selected_index_ = 0;
    keyboard_selection_ = false;
    caret_visible_ = true;
    has_anchor_rect_ = false;
}

void CandidateWindow::SetAnchorRect(const RECT& rect) {
    anchor_rect_ = rect;
    has_anchor_rect_ = rect.right >= rect.left && rect.bottom >= rect.top;
}

void CandidateWindow::ReloadStyle() {
    std::vector<wchar_t> buffer(32768);
    const DWORD length = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (!length || length >= buffer.size()) return;
    std::filesystem::path path(buffer.data(), buffer.data() + length);
    const std::wstring file = (path.parent_path() / L"heshun-ui.ini").wstring();
    style_.background = ReadStyleColor(file, L"background", style_.background);
    style_.border = ReadStyleColor(file, L"border", style_.border);
    style_.text = ReadStyleColor(file, L"text", style_.text);
    style_.annotation = ReadStyleColor(file, L"annotation", style_.annotation);
    style_.selection_background = ReadStyleColor(file, L"selection_background", style_.selection_background);
    style_.selection_text = ReadStyleColor(file, L"selection_text", style_.selection_text);
    style_.number_background = ReadStyleColor(file, L"number_background", style_.number_background);
    style_.number_text = ReadStyleColor(file, L"number_text", style_.number_text);
    style_.caret = ReadStyleColor(file, L"caret", style_.caret);
    style_.font_size = std::clamp(ReadStyleInt(file, L"font_size", style_.font_size), 6, 48);
    style_.min_width = std::clamp(ReadStyleInt(file, L"min_width", style_.min_width), 120, 800);
    style_.max_width = std::clamp(ReadStyleInt(file, L"max_width", style_.max_width), style_.min_width, 2000);
    style_.row_height = std::clamp(ReadStyleInt(file, L"row_height", style_.row_height), 16, 80);
    style_.padding = std::clamp(ReadStyleInt(file, L"padding", style_.padding), 0, 40);
    style_.corner_radius = std::clamp(ReadStyleInt(file, L"corner_radius", style_.corner_radius), 0, 32);
    wchar_t family[LF_FACESIZE]{};
    GetPrivateProfileStringW(L"candidate", L"font_family", style_.font_family.c_str(),
                             family, ARRAYSIZE(family), file.c_str());
    if (family[0]) style_.font_family = family;
}

size_t CandidateWindow::RowAtY(int y) const {
    const int scale = DpiForWindow(window_);
    const int row = MulDiv(style_.row_height, scale, 96);
    const int padding = MulDiv(style_.padding, scale, 96);
    const size_t row_count = std::min<size_t>(9, candidates_.size());
    return CandidateWindowRowIndexAtY(y, row, padding, row_count);
}

void CandidateWindow::Paint(HDC dc) {
    RECT rect{};
    GetClientRect(window_, &rect);
    HBRUSH background = CreateSolidBrush(style_.background);
    HBRUSH border = CreateSolidBrush(style_.border);
    if (background) {
        const int radius = MulDiv(style_.corner_radius, DpiForWindow(window_), 96);
        HGDIOBJ old_brush = SelectObject(dc, background);
        RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, radius * 2, radius * 2);
        SelectObject(dc, old_brush);
        DeleteObject(background);
    }
    if (border) {
        const int radius = MulDiv(style_.corner_radius, DpiForWindow(window_), 96);
        HGDIOBJ old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        HPEN pen = CreatePen(PS_SOLID, 1, style_.border);
        HGDIOBJ old_pen = pen ? SelectObject(dc, pen) : nullptr;
        RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, radius * 2, radius * 2);
        if (old_pen) SelectObject(dc, old_pen);
        if (pen) DeleteObject(pen);
        SelectObject(dc, old_brush);
        DeleteObject(border);
    }

    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, style_.text);
    HFONT font = MakeFont(window_, style_.font_size, style_.font_family);
    HGDIOBJ old = SelectObject(dc, font);

    const int scale = DpiForWindow(window_);
    const int row = MulDiv(style_.row_height, scale, 96);
    const int padding = MulDiv(style_.padding, scale, 96);
    RECT line = rect;
    line.left += padding;
    line.top += padding;
    line.right -= padding;
    line.bottom = line.top + row;

    if (candidates_.empty()) {
        SetTextColor(dc, style_.text);
        DrawTextW(dc, L"无候选（可继续输入或退格）", -1, &line,
                  DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
    }
    for (size_t i = 0; i < std::min<size_t>(9, candidates_.size()); ++i) {
        line.bottom = line.top + row;
        if (i == selected_index_) {
            RECT selection = line;
            HBRUSH selection_brush = CreateSolidBrush(style_.selection_background);
            if (selection_brush) {
                const int radius = MulDiv(style_.corner_radius, scale, 96);
                HGDIOBJ old_brush = SelectObject(dc, selection_brush);
                RoundRect(dc, selection.left, selection.top, selection.right, selection.bottom,
                          radius * 2, radius * 2);
                SelectObject(dc, old_brush);
                DeleteObject(selection_brush);
            }
        }
        const int badge_size = std::max(18, row - MulDiv(10, scale, 96));
        RECT badge{line.left + MulDiv(2, scale, 96),
                   line.top + (row - badge_size) / 2,
                   line.left + MulDiv(2, scale, 96) + badge_size,
                   line.top + (row - badge_size) / 2 + badge_size};
        HBRUSH badge_brush = CreateSolidBrush(i == selected_index_ ? style_.selection_text : style_.number_background);
        if (badge_brush) {
            const int radius = badge_size / 2;
            HGDIOBJ old_brush = SelectObject(dc, badge_brush);
            RoundRect(dc, badge.left, badge.top, badge.right, badge.bottom, radius, radius);
            SelectObject(dc, old_brush);
            DeleteObject(badge_brush);
        }
        SetTextColor(dc, i == selected_index_ ? style_.background : style_.number_text);
        const std::wstring number = std::to_wstring(i + 1);
        DrawTextW(dc, number.c_str(), -1, &badge, DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);

        const int text_left = badge.right + MulDiv(10, scale, 96);
        const std::wstring& item = candidates_[i];
        const size_t annotation_start = item.find(L"  [");
        const std::wstring word = annotation_start == std::wstring::npos ? item : item.substr(0, annotation_start);
        const std::wstring annotation = annotation_start == std::wstring::npos ? L"" : item.substr(annotation_start);
        RECT text_line = line;
        text_line.left = text_left;
        SetTextColor(dc, i == selected_index_ ? style_.selection_text : style_.text);
        DrawTextW(dc, word.c_str(), -1, &text_line, DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
        if (!annotation.empty()) {
            SIZE word_size{};
            GetTextExtentPoint32W(dc, word.c_str(), static_cast<int>(word.size()), &word_size);
            text_line.left += word_size.cx + MulDiv(8, scale, 96);
            SetTextColor(dc, style_.annotation);
            DrawTextW(dc, annotation.c_str(), -1, &text_line,
                      DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
        }
        line.top += row;
    }

    SelectObject(dc, old);
    DeleteObject(font);
}

LRESULT CALLBACK CandidateWindow::WindowProc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    CandidateWindow* self = reinterpret_cast<CandidateWindow*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        auto* create = reinterpret_cast<CREATESTRUCTW*>(lparam);
        self = static_cast<CandidateWindow*>(create->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
        self->window_ = window;
    }
    if (!self) return DefWindowProcW(window, message, wparam, lparam);
    switch (message) {
    case WM_ERASEBKGND:
        return 1;
    case WM_TIMER:
        if (wparam == kCaretTimer) {
            self->caret_visible_ = !self->caret_visible_;
            InvalidateRect(window, nullptr, FALSE);
            return 0;
        }
        break;
    case WM_PAINT: {
        PAINTSTRUCT ps{};
        HDC dc = BeginPaint(window, &ps);
        self->Paint(dc);
        EndPaint(window, &ps);
        return 0;
    }
    case WM_NCHITTEST:
        return HTCLIENT;
    case WM_MOUSEACTIVATE:
        return MA_NOACTIVATE;
    case WM_MOUSEMOVE: {
        TRACKMOUSEEVENT track{sizeof(track), TME_LEAVE, window, 0};
        TrackMouseEvent(&track);
        const size_t index = self->RowAtY(GET_Y_LPARAM(lparam));
        if (index != self->candidates_.size() && index != self->selected_index_) {
            self->selected_index_ = index;
            self->keyboard_selection_ = false;
            InvalidateRect(window, nullptr, FALSE);
        }
        return 0;
    }
    case WM_MOUSELEAVE:
        self->selected_index_ = 0;
        InvalidateRect(window, nullptr, FALSE);
        return 0;
    case WM_MOUSEWHEEL: {
        const int direction = GET_WHEEL_DELTA_WPARAM(wparam) > 0 ? -1 : 1;
        const size_t count = std::min<size_t>(9, self->candidates_.size());
        if (!count) return 0;
        const bool at_boundary = direction > 0
            ? self->selected_index_ + 1 >= count
            : self->selected_index_ == 0;
        if (at_boundary) {
            if (self->page_change_handler_) self->page_change_handler_(direction);
        } else {
            self->selected_index_ = static_cast<size_t>(
                static_cast<int>(self->selected_index_) + direction);
            self->keyboard_selection_ = false;
            InvalidateRect(window, nullptr, FALSE);
        }
        return 0;
    }
    case WM_LBUTTONUP: {
        const size_t index = self->RowAtY(GET_Y_LPARAM(lparam));
        if (index != self->candidates_.size() && self->candidate_click_handler_) {
            if (index < self->keys_.size()) self->candidate_click_handler_(self->keys_[index]);
        }
        return 0;
    }
    }
    return DefWindowProcW(window, message, wparam, lparam);
}

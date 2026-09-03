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
    if (log) log << message << '\\n';
}

constexpr wchar_t kClassName[] = L"HeshunTsfCandidateWindow";
constexpr COLORREF kBackground = RGB(255, 255, 255);
constexpr COLORREF kBorder = RGB(190, 190, 190);
constexpr COLORREF kText = RGB(30, 30, 30);
constexpr COLORREF kSelectionBackground = RGB(220, 235, 252);
constexpr COLORREF kSelectionText = RGB(0, 50, 110);
constexpr COLORREF kCaret = RGB(0, 120, 215);
constexpr UINT_PTR kCaretTimer = 1;
constexpr UINT kCaretBlinkMs = 530;
constexpr int kHeaderHeight = 28;
constexpr int kRowHeight = 25;
constexpr int kPadding = 8;

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
        wc.hbrBackground = CreateSolidBrush(kBackground);
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
    const bool has_tsf_anchor = has_anchor_rect_;
    const POINT tsf_anchor{anchor_rect_.left, anchor_rect_.bottom};
    bool has_gui_caret = false;
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
    anchor = ResolveCandidateAnchorPoint(has_tsf_anchor, tsf_anchor,
                                         has_gui_caret, anchor, fallback_anchor);
    const bool has_caret = has_tsf_anchor || has_gui_caret;
    if (!has_tsf_anchor && !anchor_window) {
        anchor_window = foreground;
        if (anchor_window) ClientToScreen(anchor_window, &anchor);
    }

    const int dpi = DpiForWindow(anchor_window ? anchor_window : window_);
    const int scale = dpi;
    const int header = MulDiv(style_.header_height, scale, 96);
        const int row = MulDiv(style_.row_height, scale, 96);
        const int padding = MulDiv(style_.padding, scale, 96);
    const int rows = static_cast<int>(std::min<size_t>(9, candidates_.size()));
    const int client_width = MulDiv(style_.width, scale, 96);
    const int client_height = header + std::max(1, rows) * row + padding * 2;
    RECT frame{0, 0, client_width, client_height};
    AdjustWindowRectExForDpi(&frame, WS_POPUP, FALSE, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST, dpi);
    const int width = frame.right - frame.left;
    const int height = frame.bottom - frame.top;

    HMONITOR monitor = MonitorFromPoint(anchor, MONITOR_DEFAULTTONEAREST);
    MONITORINFO monitor_info{sizeof(monitor_info)};
    GetMonitorInfoW(monitor, &monitor_info);
    const RECT work = monitor_info.rcWork;
    const int x = std::clamp(anchor.x, work.left, std::max(work.left, work.right - width));
    const int y = std::clamp(anchor.y + (has_caret ? 4 : 0),
                             work.top, std::max(work.top, work.bottom - height));
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

void CandidateWindow::MoveSelection(int direction) {
    const size_t count = std::min<size_t>(9, candidates_.size());
    if (!count) return;
    const int next = static_cast<int>(selected_index_) + direction;
    selected_index_ = static_cast<size_t>((next % static_cast<int>(count) + static_cast<int>(count)) % static_cast<int>(count));
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
    style_.selection_background = ReadStyleColor(file, L"selection_background", style_.selection_background);
    style_.selection_text = ReadStyleColor(file, L"selection_text", style_.selection_text);
    style_.caret = ReadStyleColor(file, L"caret", style_.caret);
    style_.font_size = std::clamp(ReadStyleInt(file, L"font_size", style_.font_size), 6, 48);
    style_.width = std::clamp(ReadStyleInt(file, L"width", style_.width), 160, 1600);
    style_.header_height = std::clamp(ReadStyleInt(file, L"header_height", style_.header_height), 16, 80);
    style_.row_height = std::clamp(ReadStyleInt(file, L"row_height", style_.row_height), 16, 80);
    style_.padding = std::clamp(ReadStyleInt(file, L"padding", style_.padding), 0, 40);
    wchar_t family[LF_FACESIZE]{};
    GetPrivateProfileStringW(L"candidate", L"font_family", style_.font_family.c_str(),
                             family, ARRAYSIZE(family), file.c_str());
    if (family[0]) style_.font_family = family;
}

size_t CandidateWindow::RowAtY(int y) const {
    const int scale = DpiForWindow(window_);
    const int header = MulDiv(style_.header_height, scale, 96);
        const int row = MulDiv(style_.row_height, scale, 96);
        const int padding = MulDiv(style_.padding, scale, 96);
    if (y < padding + header) return candidates_.size();
    const size_t index = static_cast<size_t>((y - padding - header) / std::max(1, row));
    return index < std::min<size_t>(9, candidates_.size()) ? index : candidates_.size();
}

void CandidateWindow::Paint(HDC dc) {
    RECT rect{};
    GetClientRect(window_, &rect);
    HBRUSH background = CreateSolidBrush(style_.background);
    HBRUSH border = CreateSolidBrush(style_.border);
    if (background) {
        FillRect(dc, &rect, background);
        DeleteObject(background);
    }
    if (border) {
        FrameRect(dc, &rect, border);
        DeleteObject(border);
    }

    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, style_.text);
    HFONT font = MakeFont(window_, style_.font_size, style_.font_family);
    HGDIOBJ old = SelectObject(dc, font);

    const int scale = DpiForWindow(window_);
    const int header = MulDiv(style_.header_height, scale, 96);
        const int row = MulDiv(style_.row_height, scale, 96);
        const int padding = MulDiv(style_.padding, scale, 96);
    RECT line = rect;
    line.left += padding;
    line.top += padding;
    line.right -= padding;
    line.bottom = line.top + header;
    const unsigned int page_count = page_size_ ? (total_candidates_ + page_size_ - 1) / page_size_ : 0;
    std::wstring title = L"编码: " + pending_ + L"  " + std::to_wstring(page_index_ + 1) +
                         L"/" + std::to_wstring(page_count);
    DrawTextW(dc, title.c_str(), -1, &line, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);

    // The host owns the real composition caret.  This window is an additional
    // IME-owned view, so mirror that same core cursor here as a plain vertical
    // marker in the encoding text (not a second editing cursor).
    const size_t cursor_chars = std::min<size_t>(cursor_, pending_.size());
    const std::wstring cursor_prefix = L"编码: " + pending_.substr(0, cursor_chars);
    SIZE prefix_size{};
    if (caret_visible_ && GetTextExtentPoint32W(dc, cursor_prefix.c_str(),
                                                static_cast<int>(cursor_prefix.size()), &prefix_size)) {
        const int caret_x = line.left + prefix_size.cx + MulDiv(1, scale, 96);
        const int caret_w = std::max(1, MulDiv(1, scale, 96));
        const int caret_top = line.top + MulDiv(5, scale, 96);
        const int caret_bottom = line.bottom - MulDiv(5, scale, 96);
        HBRUSH caret_brush = CreateSolidBrush(style_.caret);
        HPEN caret_pen = CreatePen(PS_SOLID, 1, style_.caret);
        if (caret_brush && caret_pen) {
            RECT caret_rect{caret_x, caret_top, caret_x + caret_w, caret_bottom};
            HGDIOBJ old_pen = SelectObject(dc, caret_pen);
            HGDIOBJ old_brush = SelectObject(dc, caret_brush);
            Rectangle(dc, caret_rect.left, caret_rect.top, caret_rect.right, caret_rect.bottom);
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
        }
        if (caret_brush) DeleteObject(caret_brush);
        if (caret_pen) DeleteObject(caret_pen);
    }

    line.top += header;
    if (candidates_.empty()) {
        line.bottom = line.top + row;
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
                FillRect(dc, &selection, selection_brush);
                DeleteObject(selection_brush);
            }
            SetTextColor(dc, style_.selection_text);
        } else {
            SetTextColor(dc, style_.text);
        }
        const std::wstring item = std::to_wstring(i + 1) + L". " + candidates_[i];
        DrawTextW(dc, item.c_str(), -1, &line, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
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

#include "candidate_window.h"
#include <windowsx.h>

#include <algorithm>
#include <cmath>
#include <mutex>

namespace {
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

HFONT MakeFont(HWND window, int points) {
    const int dpi = DpiForWindow(window);
    const int height = -MulDiv(points, dpi, 72);
    return CreateFontW(height, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                       DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                       CLEARTYPE_QUALITY, DEFAULT_PITCH | FF_DONTCARE, L"Microsoft YaHei");
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

    const int dpi = DpiForWindow(window_);
    const int scale = dpi;
    const int header = MulDiv(kHeaderHeight, scale, 96);
    const int row = MulDiv(kRowHeight, scale, 96);
    const int padding = MulDiv(kPadding, scale, 96);
    const int rows = static_cast<int>(std::min<size_t>(9, candidates_.size()));
    const int client_width = MulDiv(500, scale, 96);
    const int client_height = header + std::max(1, rows) * row + padding * 2;
    RECT frame{0, 0, client_width, client_height};
    AdjustWindowRectExForDpi(&frame, WS_POPUP, FALSE, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST, dpi);
    const int width = frame.right - frame.left;
    const int height = frame.bottom - frame.top;
    // Keep the window near the foreground caret without activating it.
    GUITHREADINFO info{sizeof(info)};
    HWND foreground = GetForegroundWindow();
    HMONITOR monitor = MonitorFromWindow(foreground ? foreground : window_, MONITOR_DEFAULTTONEAREST);
    MONITORINFO monitor_info{sizeof(monitor_info)};
    GetMonitorInfoW(monitor, &monitor_info);
    const RECT work = monitor_info.rcWork;
    if (foreground && GetGUIThreadInfo(GetWindowThreadProcessId(foreground, nullptr), &info) && info.rcCaret.right > info.rcCaret.left) {
        POINT point{info.rcCaret.left, info.rcCaret.bottom};
        ClientToScreen(info.hwndCaret ? info.hwndCaret : foreground, &point);
    const int x = std::clamp(point.x, work.left, std::max(work.left, work.right - width));
        const int y = std::clamp(point.y + 4, work.top, std::max(work.top, work.bottom - height));
        SetWindowPos(window_, HWND_TOPMOST, x, y, width, height,
                     SWP_NOACTIVATE | SWP_SHOWWINDOW);
    } else {
        const int x = std::clamp(work.left + 24, work.left, std::max(work.left, work.right - width));
        const int y = std::clamp(work.top + 24, work.top, std::max(work.top, work.bottom - height));
        SetWindowPos(window_, HWND_TOPMOST, x, y, width, height,
                     SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
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
}

size_t CandidateWindow::RowAtY(int y) const {
    const int scale = DpiForWindow(window_);
    const int header = MulDiv(kHeaderHeight, scale, 96);
    const int row = MulDiv(kRowHeight, scale, 96);
    const int padding = MulDiv(kPadding, scale, 96);
    if (y < padding + header) return candidates_.size();
    const size_t index = static_cast<size_t>((y - padding - header) / std::max(1, row));
    return index < std::min<size_t>(9, candidates_.size()) ? index : candidates_.size();
}

void CandidateWindow::Paint(HDC dc) {
    RECT rect{};
    GetClientRect(window_, &rect);
    HBRUSH background = CreateSolidBrush(kBackground);
    HBRUSH border = CreateSolidBrush(kBorder);
    if (background) {
        FillRect(dc, &rect, background);
        DeleteObject(background);
    }
    if (border) {
        FrameRect(dc, &rect, border);
        DeleteObject(border);
    }

    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, kText);
    HFONT font = MakeFont(window_, 11);
    HGDIOBJ old = SelectObject(dc, font);

    const int scale = DpiForWindow(window_);
    const int header = MulDiv(kHeaderHeight, scale, 96);
    const int row = MulDiv(kRowHeight, scale, 96);
    const int padding = MulDiv(kPadding, scale, 96);
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
        HBRUSH caret_brush = CreateSolidBrush(kCaret);
        HPEN caret_pen = CreatePen(PS_SOLID, 1, kCaret);
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
        SetTextColor(dc, kText);
        DrawTextW(dc, L"无候选（可继续输入或退格）", -1, &line,
                  DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
    }
    for (size_t i = 0; i < std::min<size_t>(9, candidates_.size()); ++i) {
        line.bottom = line.top + row;
        if (i == selected_index_) {
            RECT selection = line;
            HBRUSH selection_brush = CreateSolidBrush(kSelectionBackground);
            if (selection_brush) {
                FillRect(dc, &selection, selection_brush);
                DeleteObject(selection_brush);
            }
            SetTextColor(dc, kSelectionText);
        } else {
            SetTextColor(dc, kText);
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

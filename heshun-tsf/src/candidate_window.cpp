#include "candidate_window.h"

#include <algorithm>
#include <mutex>

namespace {
constexpr wchar_t kClassName[] = L"HeshunTsfCandidateWindow";
constexpr COLORREF kBackground = RGB(255, 255, 255);
constexpr COLORREF kBorder = RGB(190, 190, 190);
constexpr COLORREF kText = RGB(30, 30, 30);
constexpr int kHeaderHeight = 28;
constexpr int kRowHeight = 25;
constexpr int kPadding = 8;

HFONT MakeFont(int points) {
    HDC dc = GetDC(nullptr);
    const int height = -MulDiv(points, GetDeviceCaps(dc, LOGPIXELSY), 72);
    ReleaseDC(nullptr, dc);
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

void CandidateWindow::Show(std::wstring pending, std::vector<std::wstring> candidates) {
    pending_ = std::move(pending);
    candidates_ = std::move(candidates);
    if (pending_.empty() || candidates_.empty() || !EnsureWindow()) {
        Hide();
        return;
    }

    const int rows = static_cast<int>(std::min<size_t>(9, candidates_.size()));
    const int height = kHeaderHeight + rows * kRowHeight + kPadding * 2;
    // Keep the window near the foreground caret without activating it.
    GUITHREADINFO info{sizeof(info)};
    HWND foreground = GetForegroundWindow();
    if (foreground && GetGUIThreadInfo(GetWindowThreadProcessId(foreground, nullptr), &info) && info.rcCaret.right > info.rcCaret.left) {
        POINT point{info.rcCaret.left, info.rcCaret.bottom};
        ClientToScreen(info.hwndCaret ? info.hwndCaret : foreground, &point);
        SetWindowPos(window_, HWND_TOPMOST, point.x, point.y + 4, 500, height,
                     SWP_NOACTIVATE | SWP_SHOWWINDOW);
    } else {
        SetWindowPos(window_, HWND_TOPMOST, 24, 24, 500, height,
                     SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
    InvalidateRect(window_, nullptr, TRUE);
    UpdateWindow(window_);
}

void CandidateWindow::Hide() {
    if (window_) ShowWindow(window_, SW_HIDE);
    pending_.clear();
    candidates_.clear();
}

void CandidateWindow::Paint(HDC dc) {
    RECT rect{};
    GetClientRect(window_, &rect);
    FillRect(dc, &rect, CreateSolidBrush(kBackground));
    FrameRect(dc, &rect, CreateSolidBrush(kBorder));

    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, kText);
    HFONT font = MakeFont(11);
    HGDIOBJ old = SelectObject(dc, font);

    RECT line = rect;
    line.left += kPadding;
    line.top += kPadding;
    line.right -= kPadding;
    line.bottom = line.top + kHeaderHeight;
    std::wstring title = L"编码: " + pending_;
    DrawTextW(dc, title.c_str(), -1, &line, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);

    line.top += kHeaderHeight;
    for (size_t i = 0; i < std::min<size_t>(9, candidates_.size()); ++i) {
        line.bottom = line.top + kRowHeight;
        const std::wstring item = std::to_wstring(i + 1) + L". " + candidates_[i];
        DrawTextW(dc, item.c_str(), -1, &line, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
        line.top += kRowHeight;
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
    case WM_PAINT: {
        PAINTSTRUCT ps{};
        HDC dc = BeginPaint(window, &ps);
        self->Paint(dc);
        EndPaint(window, &ps);
        return 0;
    }
    case WM_NCHITTEST:
        return HTTRANSPARENT;
    }
    return DefWindowProcW(window, message, wparam, lparam);
}

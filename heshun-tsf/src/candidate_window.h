#pragma once

#include <windows.h>
#include <algorithm>
#include <functional>
#include <string>
#include <vector>

struct CandidateKey {
    unsigned int source = 0;
    unsigned int ordinal = 0;

    bool operator==(const CandidateKey& other) const {
        return source == other.source && ordinal == other.ordinal;
    }
    bool operator!=(const CandidateKey& other) const { return !(*this == other); }
};

struct CandidateWindowStyle {
    COLORREF background = RGB(248, 250, 252);
    COLORREF border = RGB(226, 232, 240);
    COLORREF text = RGB(15, 23, 42);
    COLORREF annotation = RGB(100, 116, 139);
    COLORREF selection_background = RGB(219, 234, 254);
    COLORREF selection_text = RGB(30, 64, 175);
    COLORREF number_background = RGB(226, 232, 240);
    COLORREF number_text = RGB(71, 85, 105);
    COLORREF caret = RGB(37, 99, 235);
    int font_size = 11;
    int min_width = 180;
    int max_width = 1200;
    int row_height = 30;
    int padding = 10;
    int corner_radius = 10;
    std::wstring font_family = L"Microsoft YaHei";
};

inline POINT ResolveCandidateAnchorPoint(bool has_tsf_anchor, POINT tsf_anchor,
                                         bool has_gui_caret, POINT gui_caret,
                                         POINT fallback_anchor) {
    return has_tsf_anchor ? tsf_anchor : (has_gui_caret ? gui_caret : fallback_anchor);
}

inline int CandidateWindowTopForAnchor(int caret_top, int caret_bottom, int window_height,
                                        int work_top, int work_bottom, int gap) {
    const int below = caret_bottom + gap;
    const int above = caret_top - gap - window_height;
    if (below + window_height <= work_bottom) return std::max(work_top, below);
    if (above >= work_top) return above;
    return std::clamp(below, work_top, std::max(work_top, work_bottom - window_height));
}

inline int CandidateIndexFromNumberKey(WPARAM key, size_t candidate_count) {
    if (key < '1' || key > '9') return -1;
    const int index = static_cast<int>(key - '1');
    return index < static_cast<int>(candidate_count) ? index : -1;
}

inline int DynamicCandidateClientWidth(int measured_text_width, int badge_size,
                                      int gap, int min_width, int max_width) {
    const int content_width = measured_text_width + badge_size + gap;
    return std::clamp(content_width, min_width, max_width);
}

inline int CandidateWindowContentHeight(size_t row_count, int row_height, int padding) {
    return static_cast<int>(std::max<size_t>(1, row_count)) * row_height + padding * 2;
}

inline bool ShouldShowCandidateWindow(bool has_runtime, bool has_pending,
                                      size_t candidate_count) {
    return has_runtime && has_pending && candidate_count > 0;
}

inline size_t CandidateWindowRowIndexAtY(int y, int row_height, int padding,
                                         size_t row_count) {
    if (y < padding || row_height <= 0) return row_count;
    const size_t index = static_cast<size_t>((y - padding) / row_height);
    return index < row_count ? index : row_count;
}

inline bool CandidateWindowHasPage(unsigned int page_index, unsigned int page_size,
                                   unsigned int total_candidates, int direction) {
    if (!page_size || direction == 0) return false;
    const unsigned int page_count = (total_candidates + page_size - 1) / page_size;
    return direction > 0 ? page_index + 1 < page_count : page_index > 0;
}

inline RECT CorrectCandidateAnchorRect(RECT rect, const RECT& foreground,
                                       bool has_gui_caret, POINT gui_caret,
                                       bool* corrected = nullptr) {
    const bool outside = rect.left < foreground.left || rect.left > foreground.right ||
                         rect.top < foreground.top || rect.top > foreground.bottom;
    if (corrected) *corrected = outside;
    if (!outside) return rect;
    const int offset_x = foreground.left - rect.left + (has_gui_caret ? gui_caret.x : 0);
    const int offset_y = foreground.top - rect.top + (has_gui_caret ? gui_caret.y : 0);
    rect.left += offset_x;
    rect.right += offset_x;
    rect.top += offset_y;
    rect.bottom += offset_y;
    return rect;
}

class CandidateWindow final {
public:
    CandidateWindow() = default;
    ~CandidateWindow();

    CandidateWindow(const CandidateWindow&) = delete;
    CandidateWindow& operator=(const CandidateWindow&) = delete;

    void Show(std::wstring pending, std::vector<std::wstring> candidates, std::vector<CandidateKey> keys, unsigned int page_index, unsigned int page_size, unsigned int total, unsigned int cursor);
    void Hide();
    void SetAnchorRect(const RECT& rect);
    void ReloadStyle();
    void SetCandidateClickHandler(std::function<void(CandidateKey)> handler);
    void SetPageChangeHandler(std::function<void(int)> handler);
    void MoveSelection(int direction);
    void SetSelection(size_t index);
    void UseKeyboardSelection();
    size_t selected_index() const { return selected_index_; }
    bool keyboard_selection() const { return keyboard_selection_; }

private:
    static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam, LPARAM lparam);
    bool EnsureWindow();
    void Paint(HDC dc);
    size_t RowAtY(int y) const;

    HWND window_ = nullptr;
    std::function<void(CandidateKey)> candidate_click_handler_;
    std::function<void(int)> page_change_handler_;
    std::wstring pending_;
    std::vector<std::wstring> candidates_;
    std::vector<CandidateKey> keys_;
    unsigned int page_index_ = 0;
    unsigned int page_size_ = 9;
    unsigned int total_candidates_ = 0;
    unsigned int cursor_ = 0;
    size_t selected_index_ = 0;
    bool keyboard_selection_ = false;
    bool caret_visible_ = true;
    RECT anchor_rect_{};
    bool has_anchor_rect_ = false;
    CandidateWindowStyle style_{};
};

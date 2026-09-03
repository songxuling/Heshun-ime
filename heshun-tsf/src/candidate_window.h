#pragma once

#include <windows.h>
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
    COLORREF background = RGB(255, 255, 255);
    COLORREF border = RGB(190, 190, 190);
    COLORREF text = RGB(30, 30, 30);
    COLORREF selection_background = RGB(220, 235, 252);
    COLORREF selection_text = RGB(0, 50, 110);
    COLORREF caret = RGB(0, 120, 215);
    int font_size = 11;
    int width = 500;
    int header_height = 28;
    int row_height = 25;
    int padding = 8;
    std::wstring font_family = L"Microsoft YaHei";
};

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
    void MoveSelection(int direction);
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

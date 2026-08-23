#pragma once

#include <windows.h>
#include <functional>
#include <string>
#include <vector>

class CandidateWindow final {
public:
    CandidateWindow() = default;
    ~CandidateWindow();

    CandidateWindow(const CandidateWindow&) = delete;
    CandidateWindow& operator=(const CandidateWindow&) = delete;

    void Show(std::wstring pending, std::vector<std::wstring> candidates);
    void Hide();
    void SetCandidateClickHandler(std::function<void(size_t)> handler);
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
    std::function<void(size_t)> candidate_click_handler_;
    std::wstring pending_;
    std::vector<std::wstring> candidates_;
    size_t selected_index_ = 0;
    bool keyboard_selection_ = false;
};

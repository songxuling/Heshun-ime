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

private:
    static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam, LPARAM lparam);
    bool EnsureWindow();
    void Paint(HDC dc);

    HWND window_ = nullptr;
    std::function<void(size_t)> candidate_click_handler_;
    std::wstring pending_;
    std::vector<std::wstring> candidates_;
};

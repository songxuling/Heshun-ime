#pragma once

#include <windows.h>
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

private:
    static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wparam, LPARAM lparam);
    bool EnsureWindow();
    void Paint(HDC dc);

    HWND window_ = nullptr;
    std::wstring pending_;
    std::vector<std::wstring> candidates_;
};

#pragma once

#include <windows.h>
#include <string>

struct HeshunKeyState {
    BYTE state[256]{};
    UINT scan_code = 0;
    bool extended = false;
    bool key_up = false;

    bool Shift() const { return (state[VK_SHIFT] & 0x80) != 0; }
    bool CapsLock() const { return (state[VK_CAPITAL] & 0x01) != 0; }
    bool Control() const { return (state[VK_CONTROL] & 0x80) != 0; }
    bool Alt() const { return (state[VK_MENU] & 0x80) != 0; }
    bool Windows() const { return (state[VK_LWIN] & 0x80) != 0 || (state[VK_RWIN] & 0x80) != 0; }
};

HeshunKeyState CaptureHeshunKeyState(LPARAM lparam);
bool IsHostShortcut(WPARAM key, const HeshunKeyState& state);
std::wstring TranslateHeshunKeyText(WPARAM key, const HeshunKeyState& state);

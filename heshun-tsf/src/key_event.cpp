#include "key_event.h"

HeshunKeyState CaptureHeshunKeyState(LPARAM lparam) {
    HeshunKeyState result;
    GetKeyboardState(result.state);
    result.scan_code = static_cast<UINT>((static_cast<ULONG_PTR>(lparam) >> 16) & 0xff);
    result.extended = (static_cast<ULONG_PTR>(lparam) & (1ull << 24)) != 0;
    result.key_up = (static_cast<ULONG_PTR>(lparam) & (1ull << 31)) != 0;
    return result;
}

bool IsHostShortcut(WPARAM key, const HeshunKeyState& state) {
    if (key == VK_OEM_3 && state.Control() && !state.Alt() && !state.Windows()) return false;
    return state.Control() || state.Alt() || state.Windows();
}

std::wstring TranslateHeshunKeyText(WPARAM key, const HeshunKeyState& input) {
    if (key < VK_OEM_1 || key > VK_OEM_8) return {};
    HeshunKeyState state = input;
    state.state[VK_CONTROL] = 0;
    state.state[VK_MENU] = 0;
    wchar_t buffer[8]{};
    const int written = ToUnicodeEx(static_cast<UINT>(key), state.scan_code,
                                    state.state, buffer, static_cast<int>(std::size(buffer)),
                                    0, GetKeyboardLayout(0));
    if (written <= 0) return {};
    return std::wstring(buffer, buffer + written);
}

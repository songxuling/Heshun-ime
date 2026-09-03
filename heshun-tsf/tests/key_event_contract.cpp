#include "key_event.h"

int main() {
    HeshunKeyState state{};
    state.state[VK_CONTROL] = 0x80;
    if (!IsHostShortcut('v', state)) return 1;
    if (IsHostShortcut(VK_OEM_3, state)) return 2;

    state.state[VK_MENU] = 0x80;
    if (!IsHostShortcut(VK_OEM_3, state)) return 3;

    const LPARAM lparam = static_cast<LPARAM>((0x36u << 16) | (1u << 24));
    const HeshunKeyState parsed = CaptureHeshunKeyState(lparam);
    if (parsed.scan_code != 0x36 || !parsed.extended) return 4;
    return 0;
}

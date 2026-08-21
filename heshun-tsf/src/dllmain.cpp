#include <windows.h>

HINSTANCE g_module_instance = nullptr;
volatile LONG g_server_locks = 0;
volatile LONG g_object_count = 0;

BOOL APIENTRY DllMain(HINSTANCE instance, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_module_instance = instance;
        DisableThreadLibraryCalls(instance);
    }
    return TRUE;
}

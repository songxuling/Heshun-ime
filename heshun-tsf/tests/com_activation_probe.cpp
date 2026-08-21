#include <windows.h>
#include <msctf.h>
#include <iostream>

#include "guids.h"

int wmain() {
    const HRESULT init = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (FAILED(init) && init != RPC_E_CHANGED_MODE) return 1;

    ITfTextInputProcessorEx* service = nullptr;
    const HRESULT hr = CoCreateInstance(CLSID_HeshunTextService, nullptr, CLSCTX_INPROC_SERVER,
                                        IID_PPV_ARGS(&service));
    if (SUCCEEDED(hr)) service->Release();
    if (SUCCEEDED(init)) CoUninitialize();

    if (FAILED(hr)) {
        std::wcerr << L"COM activation failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
        return 1;
    }
    std::wcout << L"TSF COM activation: OK\n";
    return 0;
}

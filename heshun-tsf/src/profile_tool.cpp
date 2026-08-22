#include <windows.h>
#include <msctf.h>
#include <iostream>

#include "guids.h"

namespace {
HRESULT GetProfiles(ITfInputProcessorProfiles** profiles) {
    *profiles = nullptr;
    return CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr,
                            CLSCTX_INPROC_SERVER, IID_PPV_ARGS(profiles));
}

HRESULT Register(const wchar_t* dll_path) {
    ITfInputProcessorProfiles* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    hr = profiles->Register(CLSID_HeshunTextService);
    if (FAILED(hr)) std::wcerr << L"Profiles::Register failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    if (SUCCEEDED(hr)) {
        ITfCategoryMgr* categories = nullptr;
        hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&categories));
        if (FAILED(hr)) std::wcerr << L"Create category manager failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
        if (SUCCEEDED(hr)) {
            hr = categories->RegisterCategory(CLSID_HeshunTextService, GUID_TFCAT_TIP_KEYBOARD,
                                              CLSID_HeshunTextService);
            if (FAILED(hr)) std::wcerr << L"Register keyboard category failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
            categories->Release();
        }
    }
    if (SUCCEEDED(hr)) {
        hr = profiles->AddLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                          GUID_PROFILE_HESHUN_ZHENGMA, kHeshunServiceName,
                                          static_cast<ULONG>(wcslen(kHeshunServiceName)),
                                          dll_path, static_cast<ULONG>(wcslen(dll_path)), 0);
        if (FAILED(hr)) std::wcerr << L"AddLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    }
    if (SUCCEEDED(hr)) {
        hr = profiles->EnableLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                             GUID_PROFILE_HESHUN_ZHENGMA, TRUE);
        if (FAILED(hr)) std::wcerr << L"EnableLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    }
    if (SUCCEEDED(hr)) {
        hr = profiles->ActivateLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                               GUID_PROFILE_HESHUN_ZHENGMA);
        if (FAILED(hr)) std::wcerr << L"ActivateLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    }
    profiles->Release();
    return hr;
}

HRESULT Activate() {
    ITfInputProcessorProfiles* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    hr = profiles->ActivateLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                           GUID_PROFILE_HESHUN_ZHENGMA);
    profiles->Release();
    if (FAILED(hr)) std::wcerr << L"ActivateLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    return hr;
}

HRESULT Unregister() {
    ITfInputProcessorProfiles* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    profiles->RemoveLanguageProfile(CLSID_HeshunTextService, kHeshunLangId, GUID_PROFILE_HESHUN_ZHENGMA);
    ITfCategoryMgr* categories = nullptr;
    if (SUCCEEDED(CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&categories)))) {
        categories->UnregisterCategory(CLSID_HeshunTextService, GUID_TFCAT_TIP_KEYBOARD,
                                       CLSID_HeshunTextService);
        categories->Release();
    }
    hr = profiles->Unregister(CLSID_HeshunTextService);
    profiles->Release();
    return static_cast<int>(hr);
}
} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc < 2 || (wcscmp(argv[1], L"register") == 0 && argc != 3) ||
        (wcscmp(argv[1], L"unregister") != 0 && wcscmp(argv[1], L"register") != 0 && wcscmp(argv[1], L"activate") != 0)) {
        std::wcerr << L"Usage: heshun_tsf_profile register <heshun_tsf.dll> | activate | unregister\n";
        return 2;
    }
    const HRESULT init = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (FAILED(init) && init != RPC_E_CHANGED_MODE) return static_cast<int>(init);
    const HRESULT hr = wcscmp(argv[1], L"register") == 0 ? Register(argv[2]) :
                       wcscmp(argv[1], L"activate") == 0 ? Activate() : Unregister();
    if (SUCCEEDED(init)) CoUninitialize();
    if (FAILED(hr)) {
        std::wcerr << L"TSF profile operation failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
        return 1;
    }
    return 0;
}

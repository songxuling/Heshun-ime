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
            const GUID categories_to_register[] = {
                GUID_TFCAT_CATEGORY_OF_TIP,
                GUID_TFCAT_TIP_KEYBOARD,
                GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
                GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            };
            for (const GUID& category : categories_to_register) {
                hr = categories->RegisterCategory(CLSID_HeshunTextService, category,
                                                  CLSID_HeshunTextService);
                if (FAILED(hr)) {
                    std::wcerr << L"Register TSF category failed: 0x" << std::hex
                               << static_cast<unsigned long>(hr) << L"\n";
                    break;
                }
            }
            categories->Release();
        }
    }
    if (SUCCEEDED(hr)) {
        hr = profiles->AddLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                          GUID_PROFILE_HESHUN, kHeshunServiceName,
                                          static_cast<ULONG>(wcslen(kHeshunServiceName)),
                                          dll_path, static_cast<ULONG>(wcslen(dll_path)), 0);
        if (FAILED(hr)) std::wcerr << L"AddLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    }
    if (SUCCEEDED(hr)) {
        hr = profiles->EnableLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                             GUID_PROFILE_HESHUN, TRUE);
        if (FAILED(hr)) std::wcerr << L"EnableLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    }
    if (SUCCEEDED(hr)) {
        hr = profiles->ActivateLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                               GUID_PROFILE_HESHUN);
    }
    profiles->Release();
    return hr;
}

HRESULT Activate() {
    ITfInputProcessorProfiles* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    hr = profiles->ActivateLanguageProfile(CLSID_HeshunTextService, kHeshunLangId, GUID_PROFILE_HESHUN);
    profiles->Release();
    if (FAILED(hr)) std::wcerr << L"ActivateLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    return hr;
}

HRESULT Unregister() {
    ITfInputProcessorProfiles* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    profiles->RemoveLanguageProfile(CLSID_HeshunTextService, kHeshunLangId, GUID_PROFILE_HESHUN);
    // Remove profiles from older two-profile installations as well.
    profiles->RemoveLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                    GUID_PROFILE_HESHUN_LEGACY_ZHENGMA);
    profiles->RemoveLanguageProfile(CLSID_HeshunTextService, kHeshunLangId,
                                    GUID_PROFILE_HESHUN_LEGACY_PINYIN);
    ITfCategoryMgr* categories = nullptr;
    if (SUCCEEDED(CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&categories)))) {
        const GUID categories_to_unregister[] = {
            GUID_TFCAT_CATEGORY_OF_TIP,
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        };
        for (const GUID& category : categories_to_unregister) {
            categories->UnregisterCategory(CLSID_HeshunTextService, category,
                                           CLSID_HeshunTextService);
        }
        categories->Release();
    }
    hr = profiles->Unregister(CLSID_HeshunTextService);
    profiles->Release();
    return static_cast<int>(hr);
}
} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc < 2 || (wcscmp(argv[1], L"register") == 0 && argc != 3) ||
        (wcscmp(argv[1], L"activate") == 0 && argc != 2) ||
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

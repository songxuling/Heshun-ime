#include <windows.h>
#include <msctf.h>
#include <iostream>

#include "guids.h"

namespace {
std::wstring ClsidString() {
    wchar_t buffer[64]{};
    StringFromGUID2(CLSID_HeshunTextService, buffer, static_cast<int>(std::size(buffer)));
    return buffer;
}

HRESULT GetProfiles(ITfInputProcessorProfileMgr** profiles) {
    *profiles = nullptr;
    return CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr,
                            CLSCTX_INPROC_SERVER, IID_PPV_ARGS(profiles));
}

HRESULT RegisterCategories() {
    ITfCategoryMgr* categories = nullptr;
    HRESULT hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_PPV_ARGS(&categories));
    if (FAILED(hr)) return hr;
    const GUID categories_to_register[] = {
        GUID_TFCAT_CATEGORY_OF_TIP,
        GUID_TFCAT_TIP_KEYBOARD,
        GUID_TFCAT_TIPCAP_SECUREMODE,
        GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
        GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
        GUID_TFCAT_TIPCAP_COMLESS,
        GUID_TFCAT_TIPCAP_WOW16,
        GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
        GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        GUID_TFCAT_PROP_AUDIODATA,
        GUID_TFCAT_PROP_INKDATA,
        GUID_TFCAT_PROPSTYLE_CUSTOM,
        GUID_TFCAT_PROPSTYLE_STATIC,
        GUID_TFCAT_PROPSTYLE_STATICCOMPACT,
        GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
        GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY,
    };
    for (const GUID& category : categories_to_register) {
        hr = categories->RegisterCategory(CLSID_HeshunTextService, category,
                                           CLSID_HeshunTextService);
        if (FAILED(hr)) break;
    }
    categories->Release();
    return hr;
}

HRESULT Register(const wchar_t* dll_path) {
    ITfInputProcessorProfileMgr* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    hr = RegisterCategories();
    if (FAILED(hr)) {
        std::wcerr << L"RegisterCategories failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
        profiles->Release();
        return hr;
    }
    std::wcout << L"Registering Heshun as a TSF profile without an HKL substitute.\n";
    const std::wstring description = kHeshunServiceName;
    const LANGID languages[] = {
        kHeshunLangId,
        kHeshunLangIdTraditional,
        kHeshunLangIdHongKong,
        kHeshunLangIdMacau,
        kHeshunLangIdSingapore,
    };
    for (const LANGID langid : languages) {
        hr = profiles->RegisterProfile(
            CLSID_HeshunTextService, langid, GUID_PROFILE_HESHUN,
            description.c_str(), static_cast<ULONG>(description.size() * sizeof(wchar_t)),
            dll_path, static_cast<ULONG>(wcslen(dll_path)), 0,
            nullptr, 0, langid == kHeshunLangId, 0);
        if (FAILED(hr)) {
            std::wcerr << L"Profiles::RegisterProfile langid=0x" << std::hex << langid
                       << L" failed: 0x" << static_cast<unsigned long>(hr) << L"\n";
            break;
        }
    }
    if (SUCCEEDED(hr)) {
        ITfCategoryMgr* categories = nullptr;
        hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&categories));
        if (FAILED(hr)) std::wcerr << L"Create category manager failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
        if (SUCCEEDED(hr)) {
            const GUID categories_to_register[] = {
                GUID_TFCAT_CATEGORY_OF_TIP,
                GUID_TFCAT_TIP_KEYBOARD,
                GUID_TFCAT_TIPCAP_SECUREMODE,
                GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
                GUID_TFCAT_TIPCAP_COMLESS,
                GUID_TFCAT_TIPCAP_WOW16,
                GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
                GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
                GUID_TFCAT_PROP_AUDIODATA,
                GUID_TFCAT_PROP_INKDATA,
                GUID_TFCAT_PROPSTYLE_CUSTOM,
                GUID_TFCAT_PROPSTYLE_STATIC,
                GUID_TFCAT_PROPSTYLE_STATICCOMPACT,
                GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
                GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY,
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
    profiles->Release();
    return hr;
}

HRESULT Activate() {
    ITfInputProcessorProfileMgr* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    hr = profiles->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR, kHeshunLangId,
                                   CLSID_HeshunTextService, GUID_PROFILE_HESHUN,
                                   nullptr, 0);
    profiles->Release();
    if (FAILED(hr)) std::wcerr << L"ActivateLanguageProfile failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n";
    return hr;
}

HRESULT Unregister() {
    ITfInputProcessorProfileMgr* profiles = nullptr;
    HRESULT hr = GetProfiles(&profiles);
    if (FAILED(hr)) { std::wcerr << L"Create profiles failed: 0x" << std::hex << static_cast<unsigned long>(hr) << L"\n"; return hr; }
    const LANGID languages[] = {
        kHeshunLangId,
        kHeshunLangIdTraditional,
        kHeshunLangIdHongKong,
        kHeshunLangIdMacau,
        kHeshunLangIdSingapore,
    };
    HRESULT remove_hr = S_OK;
    for (const LANGID langid : languages) {
        const HRESULT current = profiles->UnregisterProfile(
            CLSID_HeshunTextService, langid, GUID_PROFILE_HESHUN, 0);
        if (FAILED(current) && remove_hr == S_OK) remove_hr = current;
    }
    if (FAILED(remove_hr)) {
        std::wcerr << L"RemoveLanguageProfile failed: 0x" << std::hex
                   << static_cast<unsigned long>(remove_hr) << L"\n";
    }
    ITfCategoryMgr* categories = nullptr;
    if (SUCCEEDED(CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&categories)))) {
        const GUID categories_to_unregister[] = {
            GUID_TFCAT_CATEGORY_OF_TIP,
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_TIPCAP_SECUREMODE,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
            GUID_TFCAT_TIPCAP_COMLESS,
            GUID_TFCAT_TIPCAP_WOW16,
            GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            GUID_TFCAT_PROP_AUDIODATA,
            GUID_TFCAT_PROP_INKDATA,
            GUID_TFCAT_PROPSTYLE_CUSTOM,
            GUID_TFCAT_PROPSTYLE_STATIC,
            GUID_TFCAT_PROPSTYLE_STATICCOMPACT,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY,
        };
        for (const GUID& category : categories_to_unregister) {
            categories->UnregisterCategory(CLSID_HeshunTextService, category,
                                           CLSID_HeshunTextService);
        }
        categories->Release();
    }
    profiles->Release();
    // The CTF profile store can retain a stale per-TIP tree when the profile
    // is active or the COM server was already removed.  Clean only Heshun's
    // own HKCU tree after the API cleanup; never touch other TIPs.
    const std::wstring tip_key = L"Software\\Microsoft\\CTF\\TIP\\" +
                                 ClsidString();
    const LONG cleanup = RegDeleteTreeW(HKEY_CURRENT_USER, tip_key.c_str());
    if (cleanup != ERROR_SUCCESS && cleanup != ERROR_FILE_NOT_FOUND) {
        std::wcerr << L"CTF registry cleanup failed: 0x" << std::hex
                   << static_cast<unsigned long>(cleanup) << L"\n";
        if (SUCCEEDED(hr)) hr = HRESULT_FROM_WIN32(cleanup);
    } else {
        // Removal is idempotent: an already-unregistered COM/profile entry
        // may make the TSF API return E_FAIL even though our owned state is
        // now completely gone.
        hr = S_OK;
    }
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

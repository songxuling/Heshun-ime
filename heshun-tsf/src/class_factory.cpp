#include <windows.h>
#include <msctf.h>
#include <string>

#include "guids.h"
#include "text_service.h"

extern HINSTANCE g_module_instance;
extern volatile LONG g_server_locks;
extern volatile LONG g_object_count;

namespace {

class HeshunClassFactory final : public IClassFactory {
public:
    HeshunClassFactory() { InterlockedIncrement(&g_object_count); }
    ~HeshunClassFactory() { InterlockedDecrement(&g_object_count); }

    STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
        if (!object) return E_INVALIDARG;
        *object = nullptr;
        if (riid == IID_IUnknown || riid == IID_IClassFactory) {
            *object = static_cast<IClassFactory*>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&ref_count_); }
    STDMETHODIMP_(ULONG) Release() override {
        const ULONG count = InterlockedDecrement(&ref_count_);
        if (!count) delete this;
        return count;
    }
    STDMETHODIMP CreateInstance(IUnknown* outer, REFIID riid, void** object) override {
        if (outer) return CLASS_E_NOAGGREGATION;
        return CreateHeshunTextService(riid, object);
    }
    STDMETHODIMP LockServer(BOOL lock) override {
        if (lock) InterlockedIncrement(&g_server_locks);
        else InterlockedDecrement(&g_server_locks);
        return S_OK;
    }
private:
    LONG ref_count_ = 1;
};

std::wstring ModulePath() {
    std::wstring path(MAX_PATH, L'\0');
    DWORD count = GetModuleFileNameW(g_module_instance, path.data(), static_cast<DWORD>(path.size()));
    if (!count || count == path.size()) return {};
    path.resize(count);
    return path;
}

HRESULT SetStringValue(HKEY root, const std::wstring& subkey, const wchar_t* name, const std::wstring& value) {
    HKEY key = nullptr;
    DWORD disposition = 0;
    LONG result = RegCreateKeyExW(root, subkey.c_str(), 0, nullptr, 0, KEY_WRITE, nullptr, &key, &disposition);
    if (result != ERROR_SUCCESS) return HRESULT_FROM_WIN32(result);
    result = RegSetValueExW(key, name, 0, REG_SZ, reinterpret_cast<const BYTE*>(value.c_str()),
                            static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t)));
    RegCloseKey(key);
    return result == ERROR_SUCCESS ? S_OK : HRESULT_FROM_WIN32(result);
}

std::wstring ClsidString() {
    wchar_t buffer[64]{};
    StringFromGUID2(CLSID_HeshunTextService, buffer, static_cast<int>(std::size(buffer)));
    return buffer;
}

} // namespace

extern "C" STDAPI DllGetClassObject(REFCLSID clsid, REFIID riid, void** object) {
    if (clsid != CLSID_HeshunTextService) return CLASS_E_CLASSNOTAVAILABLE;
    auto* factory = new (std::nothrow) HeshunClassFactory();
    if (!factory) return E_OUTOFMEMORY;
    const HRESULT hr = factory->QueryInterface(riid, object);
    factory->Release();
    return hr;
}

extern "C" STDAPI DllCanUnloadNow() {
    return (g_object_count == 0 && g_server_locks == 0) ? S_OK : S_FALSE;
}

extern "C" STDAPI DllRegisterServer() {
    const std::wstring clsid = ClsidString();
    const std::wstring module = ModulePath();
    if (module.empty()) return E_FAIL;
    const std::wstring base = L"Software\\Classes\\CLSID\\" + clsid;
    HRESULT hr = SetStringValue(HKEY_CURRENT_USER, base, nullptr, kHeshunServiceName);
    if (FAILED(hr)) return hr;
    hr = SetStringValue(HKEY_CURRENT_USER, base + L"\\InprocServer32", nullptr, module);
    if (FAILED(hr)) return hr;
    return SetStringValue(HKEY_CURRENT_USER, base + L"\\InprocServer32", L"ThreadingModel", L"Apartment");
}

extern "C" STDAPI DllUnregisterServer() {
    const std::wstring base = L"Software\\Classes\\CLSID\\" + ClsidString();
    const LONG result = RegDeleteTreeW(HKEY_CURRENT_USER, base.c_str());
    return (result == ERROR_SUCCESS || result == ERROR_FILE_NOT_FOUND) ? S_OK : HRESULT_FROM_WIN32(result);
}

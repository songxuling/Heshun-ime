#include <windows.h>
#include <msctf.h>
#include <shlwapi.h>
#include <string>
#include <cstring>
#include <vector>

#include "guids.h"
#include "text_service.h"

extern HINSTANCE g_module_instance;
extern volatile LONG g_object_count;

namespace {

class CommitEditSession final : public ITfEditSession {
public:
    CommitEditSession(ITfContext* context, std::wstring text) : context_(context), text_(std::move(text)) {
        context_->AddRef();
    }
    ~CommitEditSession() { context_->Release(); }

    STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
        if (!object) return E_INVALIDARG;
        *object = nullptr;
        if (riid == IID_IUnknown || riid == IID_ITfEditSession) {
            *object = static_cast<ITfEditSession*>(this);
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

    STDMETHODIMP DoEditSession(TfEditCookie ec) override {
        ITfInsertAtSelection* insert = nullptr;
        HRESULT hr = context_->QueryInterface(IID_PPV_ARGS(&insert));
        if (FAILED(hr)) return hr;
        ITfRange* inserted = nullptr;
        hr = insert->InsertTextAtSelection(ec, TF_IAS_NOQUERY, text_.c_str(),
                                            static_cast<LONG>(text_.size()), &inserted);
        if (inserted) inserted->Release();
        insert->Release();
        return hr;
    }

private:
    LONG ref_count_ = 1;
    ITfContext* context_ = nullptr;
    std::wstring text_;
};

std::wstring Utf8ToUtf16(const char* value) {
    if (!value || !*value) return {};
    const int length = static_cast<int>(std::strlen(value));
    const int needed = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value, length, nullptr, 0);
    if (!needed) return {};
    std::wstring result(static_cast<size_t>(needed), L'\0');
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value, length, result.data(), needed)) return {};
    return result;
}

std::string ModuleDirectory() {
    std::vector<wchar_t> path(MAX_PATH);
    DWORD length = 0;
    for (;;) {
        length = GetModuleFileNameW(g_module_instance, path.data(), static_cast<DWORD>(path.size()));
        if (length == 0) return {};
        if (length < path.size() - 1) break;
        path.resize(path.size() * 2);
    }
    std::wstring directory(path.data(), length);
    const auto slash = directory.find_last_of(L"\\/");
    if (slash == std::wstring::npos) return {};
    directory.resize(slash);
    const int bytes = WideCharToMultiByte(CP_UTF8, 0, directory.c_str(), -1, nullptr, 0, nullptr, nullptr);
    if (!bytes) return {};
    std::string result(static_cast<size_t>(bytes - 1), '\0');
    WideCharToMultiByte(CP_UTF8, 0, directory.c_str(), -1, result.data(), bytes, nullptr, nullptr);
    return result;
}

} // namespace

HeshunTextService::HeshunTextService() { InterlockedIncrement(&g_object_count); }
HeshunTextService::~HeshunTextService() {
    Deactivate();
    InterlockedDecrement(&g_object_count);
}

STDMETHODIMP HeshunTextService::QueryInterface(REFIID riid, void** object) {
    if (!object) return E_INVALIDARG;
    *object = nullptr;
    if (riid == IID_IUnknown || riid == IID_ITfTextInputProcessor || riid == IID_ITfTextInputProcessorEx) {
        *object = static_cast<ITfTextInputProcessorEx*>(this);
    } else if (riid == IID_ITfKeyEventSink) {
        *object = static_cast<ITfKeyEventSink*>(this);
    } else {
        return E_NOINTERFACE;
    }
    AddRef();
    return S_OK;
}

STDMETHODIMP_(ULONG) HeshunTextService::AddRef() { return InterlockedIncrement(&ref_count_); }
STDMETHODIMP_(ULONG) HeshunTextService::Release() {
    const ULONG count = InterlockedDecrement(&ref_count_);
    if (!count) delete this;
    return count;
}

STDMETHODIMP HeshunTextService::Activate(ITfThreadMgr* thread_mgr, TfClientId client_id) {
    return ActivateEx(thread_mgr, client_id, 0);
}

STDMETHODIMP HeshunTextService::ActivateEx(ITfThreadMgr* thread_mgr, TfClientId client_id, DWORD) {
    if (!thread_mgr) return E_INVALIDARG;
    if (thread_mgr_) return S_FALSE;
    thread_mgr_ = thread_mgr;
    thread_mgr_->AddRef();
    client_id_ = client_id;
    if (!LoadEngine()) { Deactivate(); return E_FAIL; }

    ITfKeystrokeMgr* keystrokes = nullptr;
    HRESULT hr = thread_mgr_->QueryInterface(IID_PPV_ARGS(&keystrokes));
    if (SUCCEEDED(hr)) {
        hr = keystrokes->AdviseKeyEventSink(client_id_, static_cast<ITfKeyEventSink*>(this), TRUE);
        keystrokes->Release();
    }
    if (FAILED(hr)) { Deactivate(); }
    return hr;
}

STDMETHODIMP HeshunTextService::Deactivate() {
    if (thread_mgr_) {
        ITfKeystrokeMgr* keystrokes = nullptr;
        if (SUCCEEDED(thread_mgr_->QueryInterface(IID_PPV_ARGS(&keystrokes)))) {
            keystrokes->UnadviseKeyEventSink(client_id_);
            keystrokes->Release();
        }
    }
    SaveUserDictionary();
    FreeEngine();
    if (thread_mgr_) { thread_mgr_->Release(); thread_mgr_ = nullptr; }
    client_id_ = TF_CLIENTID_NULL;
    return S_OK;
}

bool HeshunTextService::LoadEngine() {
    const std::string directory = ModuleDirectory();
    if (directory.empty()) return false;
    const std::string schema = directory + "\\schemas\\zhengma66.schema.yaml";
    engine_ = hs_engine_load_schema(schema.c_str());
    if (!engine_) return false;
    session_ = hs_session_new(engine_);
    return session_ != nullptr;
}

void HeshunTextService::SaveUserDictionary() {
    if (!engine_) return;
    const std::string directory = ModuleDirectory();
    if (!directory.empty()) {
        const std::string user_dict = directory + "\\data\\zhengma66.userdb.json";
        hs_user_dict_save(engine_, user_dict.c_str());
    }
}

void HeshunTextService::FreeEngine() {
    if (session_) { hs_session_free(session_); session_ = nullptr; }
    if (engine_) { hs_engine_free(engine_); engine_ = nullptr; }
}

bool HeshunTextService::IsHandledKey(WPARAM key) const {
        if (key >= 'A' && key <= 'Z') return true;
        if (key >= 'a' && key <= 'z') return true;
        if (key == VK_BACK || key == VK_ESCAPE || key == VK_SPACE) return true;
        return key >= '1' && key <= '9';
}

bool HeshunTextService::FeedKey(WPARAM key, char** committed) {
    if (!session_) return false;
    *committed = nullptr;
    if (key >= 'A' && key <= 'Z') return hs_feed(session_, static_cast<char>(key - 'A' + 'a'), committed) != 0;
    if (key >= 'a' && key <= 'z') return hs_feed(session_, static_cast<char>(key), committed) != 0;
    if (key == VK_BACK) { hs_backspace(session_); return true; }
    if (key == VK_ESCAPE) { hs_clear(session_); return true; }
    if (key == VK_SPACE) { *committed = hs_select_first(session_); return true; }
    if (key >= '1' && key <= '9') { *committed = hs_select(session_, static_cast<int>(key - '0')); return true; }
    return false;
}

STDMETHODIMP HeshunTextService::OnSetFocus(BOOL) { return S_OK; }
STDMETHODIMP HeshunTextService::OnTestKeyDown(ITfContext*, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = IsHandledKey(wparam) ? TRUE : FALSE;
    return S_OK;
}

STDMETHODIMP HeshunTextService::OnKeyDown(ITfContext* context, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = FALSE;
    if (!IsHandledKey(wparam)) return S_OK;
    char* committed = nullptr;
    if (!FeedKey(wparam, &committed)) return S_OK;
    *eaten = TRUE;
    if (committed) {
        const HRESULT hr = CommitText(context, committed);
        hs_str_free(committed);
        return hr;
    }
    return S_OK;
}

STDMETHODIMP HeshunTextService::OnTestKeyUp(ITfContext*, WPARAM, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = FALSE;
    return S_OK;
}
STDMETHODIMP HeshunTextService::OnKeyUp(ITfContext*, WPARAM, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = FALSE;
    return S_OK;
}
STDMETHODIMP HeshunTextService::OnPreservedKey(ITfContext*, REFGUID, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = FALSE;
    return S_OK;
}

HRESULT HeshunTextService::CommitText(ITfContext* context, const char* utf8) {
    if (!context) return E_INVALIDARG;
    std::wstring text = Utf8ToUtf16(utf8);
    if (text.empty()) return S_OK;
    auto* edit = new (std::nothrow) CommitEditSession(context, std::move(text));
    if (!edit) return E_OUTOFMEMORY;
    HRESULT session_hr = E_FAIL;
    const HRESULT hr = context->RequestEditSession(client_id_, edit, TF_ES_SYNC | TF_ES_READWRITE, &session_hr);
    edit->Release();
    return FAILED(hr) ? hr : session_hr;
}

HRESULT CreateHeshunTextService(REFIID riid, void** object) {
    auto* service = new (std::nothrow) HeshunTextService();
    if (!service) return E_OUTOFMEMORY;
    const HRESULT hr = service->QueryInterface(riid, object);
    service->Release();
    return hr;
}

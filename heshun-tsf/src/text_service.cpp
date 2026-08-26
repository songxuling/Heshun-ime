#include <windows.h>
#include <msctf.h>
#include <shlwapi.h>
#include <string>
#include <cstring>
#include <fstream>
#include <filesystem>
#include <sstream>
#include <iomanip>
#include <vector>
#include <strsafe.h>

#include "guids.h"
#include "text_service.h"

extern HINSTANCE g_module_instance;
extern volatile LONG g_object_count;

namespace {

void Trace(const std::string& message);
std::string Hr(HRESULT hr);

class HeshunLangBarItem final : public ITfLangBarItemButton {
public:
    explicit HeshunLangBarItem(HeshunTextService* service) : service_(service) {
        service_->AddRef();
        InterlockedIncrement(&g_object_count);
    }
    ~HeshunLangBarItem() {
        service_->Release();
        InterlockedDecrement(&g_object_count);
    }

    STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
        if (!object) return E_INVALIDARG;
        *object = nullptr;
        if (riid == IID_IUnknown || riid == IID_ITfLangBarItem || riid == IID_ITfLangBarItemButton) {
            *object = static_cast<ITfLangBarItemButton*>(this);
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
    STDMETHODIMP GetInfo(TF_LANGBARITEMINFO* info) override {
        if (!info) return E_INVALIDARG;
        Trace("LangBarItem: GetInfo");
        ZeroMemory(info, sizeof(*info));
        info->clsidService = CLSID_HeshunTextService;
        info->guidItem = GUID_LANGBAR_ITEM_HESHUN;
        info->dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY;
        info->ulSort = 1;
        StringCchCopyW(info->szDescription, ARRAYSIZE(info->szDescription), L"heshun");
        return S_OK;
    }
    STDMETHODIMP GetStatus(DWORD* status) override {
        if (!status) return E_INVALIDARG;
        Trace("LangBarItem: GetStatus");
        *status = 0;
        return S_OK;
    }
    STDMETHODIMP Show(BOOL show) override { Trace(show ? "LangBarItem: Show" : "LangBarItem: Hide"); return S_OK; }
    STDMETHODIMP GetTooltipString(BSTR* tooltip) override {
        if (!tooltip) return E_INVALIDARG;
        const wchar_t* mode = service_->ascii_mode() ? L"英文" : L"中文";
        const wchar_t* schema = service_->IsPinyinMode() ? L"全拼" : L"郑码";
        std::wstring value = std::wstring(L"heshun ") + mode + L" / " + schema +
                             (service_->IsPinyinMode() ? L" (点击切换郑码)" : L" (点击切换全拼)");
        *tooltip = SysAllocString(value.c_str());
        return *tooltip ? S_OK : E_OUTOFMEMORY;
    }
    STDMETHODIMP OnClick(TfLBIClick click, POINT, const RECT*) override {
        if (click == TF_LBI_CLK_LEFT) service_->ToggleInputMethodFromLangBar();
        return S_OK;
    }
    STDMETHODIMP InitMenu(ITfMenu*) override { return E_NOTIMPL; }
    STDMETHODIMP OnMenuSelect(UINT) override { return E_NOTIMPL; }
    STDMETHODIMP GetIcon(HICON* icon) override {
        if (!icon) return E_INVALIDARG;
        *icon = LoadIconW(g_module_instance, MAKEINTRESOURCEW(1));
        Trace(*icon ? "LangBarItem: GetIcon ok" : "LangBarItem: GetIcon failed");
        return *icon ? S_OK : HRESULT_FROM_WIN32(GetLastError());
    }
    STDMETHODIMP GetText(BSTR* text) override {
        if (!text) return E_INVALIDARG;
        *text = SysAllocString(service_->ascii_mode() ? L"英" :
                               (service_->IsPinyinMode() ? L"全" : L"郑"));
        Trace(*text ? "LangBarItem: GetText ok" : "LangBarItem: GetText failed");
        return *text ? S_OK : E_OUTOFMEMORY;
    }

private:
    LONG ref_count_ = 1;
    HeshunTextService* service_ = nullptr;
};

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
        Trace("CommitEditSession: begin");
        TF_SELECTION selection{};
        ULONG fetched = 0;
        HRESULT hr = context_->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &selection, &fetched);
        if (FAILED(hr) || fetched != 1 || !selection.range) {
            Trace("CommitEditSession: GetSelection failed " + Hr(FAILED(hr) ? hr : E_FAIL));
            return FAILED(hr) ? hr : E_FAIL;
        }

        hr = selection.range->SetText(ec, 0, text_.c_str(), static_cast<LONG>(text_.size()));
        if (SUCCEEDED(hr)) hr = selection.range->Collapse(ec, TF_ANCHOR_END);
        if (SUCCEEDED(hr)) hr = context_->SetSelection(ec, 1, &selection);
        selection.range->Release();
        Trace(FAILED(hr) ? "CommitEditSession: selection SetText failed " + Hr(hr) :
                           "CommitEditSession: selection SetText succeeded");
        return hr;
    }

private:
    LONG ref_count_ = 1;
    ITfContext* context_ = nullptr;
    std::wstring text_;
};

class CompositionEditSession final : public ITfEditSession {
public:
    enum class Action { Update, Commit, Cancel };

    CompositionEditSession(HeshunTextService* service, ITfContext* context, Action action, std::wstring text = {})
        : service_(service), context_(context), action_(action), text_(std::move(text)) {
        service_->AddRef();
        context_->AddRef();
    }
    ~CompositionEditSession() { context_->Release(); service_->Release(); }

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
        Trace("CompositionEditSession: DoEditSession action=" + std::to_string(static_cast<int>(action_)));
        if (action_ == Action::Cancel) return End(ec, true);
        if (action_ == Action::Commit) return Commit(ec);
        return Update(ec);
    }

private:
    HRESULT Update(TfEditCookie ec) {
        Trace("CompositionEditSession: update begin");
        if (text_.empty()) return End(ec, true);
        ITfComposition* composition = service_->composition();
        HRESULT hr = S_OK;
        if (!composition) {
            TF_SELECTION selection{};
            ULONG fetched = 0;
            hr = context_->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &selection, &fetched);
            if (FAILED(hr) || fetched != 1 || !selection.range) return FAILED(hr) ? hr : E_FAIL;
            ITfContextComposition* contexts = nullptr;
            hr = context_->QueryInterface(IID_PPV_ARGS(&contexts));
            if (SUCCEEDED(hr)) {
                hr = contexts->StartComposition(ec, selection.range, nullptr, &composition);
                contexts->Release();
            }
            selection.range->Release();
            if (FAILED(hr)) return hr;
            service_->SetComposition(composition);
            composition->Release();
            composition = service_->composition();
        }
        ITfRange* range = nullptr;
        hr = composition->GetRange(&range);
        if (SUCCEEDED(hr)) {
            hr = range->SetText(ec, 0, text_.c_str(), static_cast<LONG>(text_.size()));
            Trace("Composition: SetText " + Hr(hr));
            if (SUCCEEDED(hr) && service_->display_attribute_atom() != TF_INVALID_GUIDATOM) {
                ITfProperty* attribute = nullptr;
                hr = context_->GetProperty(GUID_PROP_ATTRIBUTE, &attribute);
                if (SUCCEEDED(hr)) {
                    VARIANT value;
                    VariantInit(&value);
                    value.vt = VT_I4;
                    value.lVal = static_cast<LONG>(service_->display_attribute_atom());
                    hr = attribute->SetValue(ec, range, &value);
                    Trace("Composition: SetAttribute " + Hr(hr));
                    VariantClear(&value);
                    attribute->Release();
                }
            }
            if (SUCCEEDED(hr)) {
                ITfRange* caret_range = nullptr;
                const HRESULT clone_hr = range->Clone(&caret_range);
                if (SUCCEEDED(clone_hr)) {
                    const HRESULT collapse_hr = caret_range->Collapse(ec, TF_ANCHOR_END);
                    if (SUCCEEDED(collapse_hr)) {
                        TF_SELECTION selection{};
                        selection.range = caret_range;
                        selection.style.ase = TF_AE_NONE;
                        const HRESULT selection_hr = context_->SetSelection(ec, 1, &selection);
                        Trace("Composition: SetCaret " + Hr(selection_hr));
                        if (FAILED(selection_hr)) hr = selection_hr;
                    } else {
                        Trace("Composition: CollapseCaret " + Hr(collapse_hr));
                        hr = collapse_hr;
                    }
                    caret_range->Release();
                } else {
                    Trace("Composition: CloneCaret " + Hr(clone_hr));
                    hr = clone_hr;
                }
            }
            range->Release();
        }
        Trace(FAILED(hr) ? "Composition: update failed " + Hr(hr) : "Composition: updated");
        return hr;
    }

    HRESULT Commit(TfEditCookie ec) {
        ITfComposition* composition = service_->composition();
        if (!composition) return E_FAIL;
        ITfRange* range = nullptr;
        HRESULT hr = composition->GetRange(&range);
        if (SUCCEEDED(hr)) {
            hr = range->SetText(ec, 0, text_.c_str(), static_cast<LONG>(text_.size()));
            if (SUCCEEDED(hr)) {
                ITfProperty* attribute = nullptr;
                const HRESULT property_hr = context_->GetProperty(GUID_PROP_ATTRIBUTE, &attribute);
                if (SUCCEEDED(property_hr)) {
                    const HRESULT clear_hr = attribute->Clear(ec, range);
                    Trace("Composition: ClearAttribute " + Hr(clear_hr));
                    attribute->Release();
                }
            }
            if (SUCCEEDED(hr)) hr = range->Collapse(ec, TF_ANCHOR_END);
            if (SUCCEEDED(hr)) {
                TF_SELECTION selection{};
                selection.range = range;
                selection.style.ase = TF_AE_NONE;
                hr = context_->SetSelection(ec, 1, &selection);
            }
            range->Release();
        }
        if (SUCCEEDED(hr)) hr = composition->EndComposition(ec);
        if (SUCCEEDED(hr)) service_->ClearComposition();
        Trace(FAILED(hr) ? "Composition: commit failed " + Hr(hr) : "Composition: committed");
        return hr;
    }

    HRESULT End(TfEditCookie ec, bool erase) {
        ITfComposition* composition = service_->composition();
        if (!composition) return S_OK;
        HRESULT hr = S_OK;
        if (erase) {
            ITfRange* range = nullptr;
            hr = composition->GetRange(&range);
            if (SUCCEEDED(hr)) {
                ITfProperty* attribute = nullptr;
                const HRESULT property_hr = context_->GetProperty(GUID_PROP_ATTRIBUTE, &attribute);
                if (SUCCEEDED(property_hr)) {
                    const HRESULT clear_hr = attribute->Clear(ec, range);
                    Trace("Composition: ClearAttribute " + Hr(clear_hr));
                    attribute->Release();
                }
                hr = range->SetText(ec, 0, L"", 0);
                range->Release();
            }
        }
        if (SUCCEEDED(hr)) hr = composition->EndComposition(ec);
        if (SUCCEEDED(hr)) service_->ClearComposition();
        Trace(FAILED(hr) ? "Composition: cancel failed " + Hr(hr) : "Composition: cancelled");
        return hr;
    }

    LONG ref_count_ = 1;
    HeshunTextService* service_ = nullptr;
    ITfContext* context_ = nullptr;
    Action action_;
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

std::wstring Utf8ViewToUtf16(hs_text_view view) {
    if (!view.ptr || !view.len) return {};
    std::string value(reinterpret_cast<const char*>(view.ptr), view.len);
    return Utf8ToUtf16(value.c_str());
}

std::string Utf8ViewToString(hs_text_view view) {
    if (!view.ptr || !view.len) return {};
    return std::string(reinterpret_cast<const char*>(view.ptr), view.len);
}

std::vector<std::wstring> ParseCandidateWords(const char* value) {
    std::vector<std::wstring> result;
    if (!value) return result;
    std::string encoded(value);
    size_t begin = 0;
    while (begin <= encoded.size()) {
        const size_t end = encoded.find('\x02', begin);
        const std::string entry = encoded.substr(begin, end == std::string::npos ? std::string::npos : end - begin);
        const size_t separator = entry.find('\x01');
        const std::string word = entry.substr(0, separator);
        const std::string code = separator == std::string::npos ? "" : entry.substr(separator + 1);
        std::wstring display = Utf8ToUtf16(word.c_str());
        if (!code.empty()) display += L"  [" + Utf8ToUtf16(code.c_str()) + L"]";
        if (!display.empty()) result.push_back(std::move(display));
        if (end == std::string::npos) break;
        begin = end + 1;
    }
    return result;
}

std::filesystem::path ModuleDirectoryPath() {
    std::vector<wchar_t> path(MAX_PATH);
    DWORD length = 0;
    for (;;) {
        length = GetModuleFileNameW(g_module_instance, path.data(), static_cast<DWORD>(path.size()));
        if (length == 0) return {};
        if (length < path.size() - 1) break;
        path.resize(path.size() * 2);
    }
    std::filesystem::path module(path.data(), path.data() + length);
    return module.parent_path();
}

std::string ModuleDirectory() {
    const auto directory = ModuleDirectoryPath();
    if (directory.empty()) return {};
    const std::wstring wide = directory.wstring();
    const int bytes = WideCharToMultiByte(CP_UTF8, 0, wide.data(), static_cast<int>(wide.size()), nullptr, 0, nullptr, nullptr);
    if (!bytes) return {};
    std::string result(static_cast<size_t>(bytes), '\0');
    if (!WideCharToMultiByte(CP_UTF8, 0, wide.data(), static_cast<int>(wide.size()), result.data(), bytes, nullptr, nullptr)) return {};
    return result;
}

void Trace(const std::string& message) {
    static std::filesystem::path log_path;
    if (log_path.empty()) {
        const auto module_dir = ModuleDirectoryPath();
        if (!module_dir.empty()) {
            const auto candidate = module_dir / L"heshun-tsf.log";
            std::ofstream probe(candidate, std::ios::app);
            if (probe) log_path = candidate;
        }
        if (log_path.empty()) {
            std::vector<wchar_t> temp(MAX_PATH);
            const DWORD length = GetTempPathW(static_cast<DWORD>(temp.size()), temp.data());
            if (length && length < temp.size()) {
                log_path = std::filesystem::path(temp.data()) /
                           (L"heshun-tsf-" + std::to_wstring(GetCurrentProcessId()) + L".log");
            }
        }
    }
    if (log_path.empty()) return;
    std::ofstream log(log_path, std::ios::app);
    if (log) log << message << '\n';
}

std::string Hr(HRESULT hr) {
    std::ostringstream out;
    out << "0x" << std::uppercase << std::hex << static_cast<unsigned long>(hr);
    return out.str();
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
    } else if (riid == IID_ITfDisplayAttributeProvider) {
        *object = static_cast<ITfDisplayAttributeProvider*>(this);
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
    if (!thread_mgr) { Trace("ActivateEx: missing thread manager"); return E_INVALIDARG; }
    if (thread_mgr_) { Trace("ActivateEx: already active"); return S_FALSE; }
    thread_mgr_ = thread_mgr;
    thread_mgr_->AddRef();
    client_id_ = client_id;
    if (!LoadEngine()) { Trace("ActivateEx: heshun engine/schema load failed"); Deactivate(); return E_FAIL; }
    Trace("ActivateEx: engine loaded");

    HRESULT hr = E_FAIL;
    ITfCategoryMgr* categories = nullptr;
    hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER,
                           IID_PPV_ARGS(&categories));
    if (SUCCEEDED(hr)) {
        hr = categories->RegisterGUID(GUID_DISPLAYATTRIBUTE_HESHUN_PREEDIT,
                                      &display_attribute_atom_);
        categories->Release();
    }
    if (FAILED(hr)) {
        Trace("ActivateEx: display attribute atom unavailable " + Hr(hr));
        display_attribute_atom_ = TF_INVALID_GUIDATOM;
    }

    ITfKeystrokeMgr* keystrokes = nullptr;
    hr = thread_mgr_->QueryInterface(IID_PPV_ARGS(&keystrokes));
    Trace("ActivateEx: keystroke manager query " + Hr(hr) +
          " client_id=" + std::to_string(client_id_) +
          " sink=" + std::to_string(reinterpret_cast<uintptr_t>(static_cast<ITfKeyEventSink*>(this))));
    if (SUCCEEDED(hr)) {
        hr = keystrokes->AdviseKeyEventSink(client_id_, static_cast<ITfKeyEventSink*>(this), TRUE);
        Trace("ActivateEx: advise key sink " + Hr(hr));
        keystrokes->Release();
    }
    if (SUCCEEDED(hr)) {
        HRESULT langbar_hr = thread_mgr_->QueryInterface(IID_PPV_ARGS(&langbar_mgr_));
        Trace("ActivateEx: language bar manager query " + Hr(langbar_hr));
        if (SUCCEEDED(langbar_hr)) {
            auto* item = new (std::nothrow) HeshunLangBarItem(this);
            if (!item) {
                langbar_hr = E_OUTOFMEMORY;
            }
            else {
                langbar_hr = langbar_mgr_->AddItem(item);
                Trace("ActivateEx: language bar add item " + Hr(langbar_hr));
                if (SUCCEEDED(langbar_hr)) langbar_item_ = item;
                item->Release();
            }
            if (FAILED(langbar_hr)) {
                Trace("ActivateEx: language bar unavailable; continuing without language bar");
                langbar_mgr_->Release();
                langbar_mgr_ = nullptr;
            }
        } else {
            Trace("ActivateEx: language bar unavailable; continuing without language bar");
        }
    }
    if (FAILED(hr)) { Trace("ActivateEx: activation setup failed"); Deactivate(); }
    else Trace("ActivateEx: key sink and language bar advised");
    return hr;
}

STDMETHODIMP HeshunTextService::Deactivate() {
    if (langbar_mgr_) {
        if (langbar_item_) langbar_mgr_->RemoveItem(langbar_item_);
        langbar_item_ = nullptr;
        langbar_mgr_->Release();
        langbar_mgr_ = nullptr;
    }
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

const char* HeshunTextService::ActiveSchemaId() const {
    return pinyin_mode_ ? "pinyin_full" : "zhengma66";
}

const char* HeshunTextService::ActiveSchemaFile() const {
    return std::strcmp(ActiveSchemaId(), "pinyin_full") == 0 ? "pinyin_full.schema.yaml" : "zhengma66.schema.yaml";
}

const char* HeshunTextService::ActiveUserDictFile() const {
    return std::strcmp(ActiveSchemaId(), "pinyin_full") == 0 ? "pinyin_full.userdb.json" : "zhengma66.userdb.json";
}

bool HeshunTextService::LoadEngine() {
    const std::string directory = ModuleDirectory();
    if (directory.empty()) { Trace("LoadEngine: module directory unavailable"); return false; }
    const std::string schema = directory + "\\schemas\\" + ActiveSchemaFile();
    Trace("LoadEngine schema: " + schema);
    runtime_ = hs_runtime_new_schema(schema.c_str());
    if (!runtime_) { Trace("LoadEngine: hs_runtime_new_schema returned null"); return false; }
    pending_.clear();
    candidates_.clear();
    page_index_ = 0;
    total_candidates_ = 0;
    has_selected_key_ = false;
    return true;
}

bool HeshunTextService::DispatchRuntime(unsigned int opcode, long long value, CandidateKey key) {
    if (!runtime_) return false;
    hs_runtime_event_t event{};
    event.opcode = opcode;
    event.value = value;
    event.source = key.source;
    event.ordinal = key.ordinal;
    hs_handle* result = hs_runtime_event(runtime_, &event);
    if (!result) return false;
    const hs_runtime_result* view = hs_runtime_result_view(result);
    if (!view) { hs_runtime_result_free(result); return false; }

    last_committed_ = Utf8ViewToString(view->committed);
    pending_ = Utf8ViewToUtf16(view->pending);
    cursor_ = std::min<unsigned int>(view->cursor, static_cast<unsigned int>(pending_.size()));
    candidates_.clear();
    for (unsigned int i = 0; i < view->candidate_count; ++i) {
        const hs_candidate_view& candidate = view->candidates[i];
        candidates_.push_back(RuntimeCandidate{
            CandidateKey{candidate.source, candidate.ordinal},
            Utf8ViewToUtf16(candidate.word),
            Utf8ViewToUtf16(candidate.annotation),
            Utf8ViewToUtf16(candidate.label),
        });
    }
    page_index_ = view->page_index;
    page_size_ = view->page_size;
    total_candidates_ = view->total_candidates;
    has_previous_page_ = view->has_previous != 0;
    has_next_page_ = view->has_next != 0;
    has_selected_key_ = view->selected_source != 0;
    selected_key_ = CandidateKey{view->selected_source, view->selected_ordinal};
    ascii_mode_ = view->ascii_mode != 0;
    const bool consumed = view->disposition != 0;
    std::ostringstream event_log;
    event_log << "RuntimeEvent: profile=heshun opcode=" << opcode
              << " disposition=" << view->disposition
              << " composition=" << view->composition
              << " pending_len=" << view->pending.len
              << " candidates=" << view->candidate_count
              << " page=" << view->page_index << "/" << view->page_size
              << " total=" << view->total_candidates
              << " selected=" << view->selected_source << ":" << view->selected_ordinal
              << " committed_len=" << view->committed.len;
    Trace(event_log.str());
    hs_runtime_result_free(result);
    return consumed;
}

void HeshunTextService::SaveUserDictionary() {
    if (!runtime_) return;
    const std::string directory = ModuleDirectory();
    if (!directory.empty()) {
        const std::string user_dict = directory + "\\data\\" + ActiveUserDictFile();
        hs_runtime_user_dict_save(runtime_, user_dict.c_str());
    }
}

void HeshunTextService::SetComposition(ITfComposition* composition) {
    if (composition == composition_) return;
    ClearComposition();
    composition_ = composition;
    if (composition_) composition_->AddRef();
}

void HeshunTextService::ClearComposition() {
    if (composition_) {
        composition_->Release();
        composition_ = nullptr;
    }
}

void HeshunTextService::FreeEngine() {
    if (active_context_ && composition_) {
        const HRESULT cancel_hr = CancelComposition(active_context_);
        Trace(FAILED(cancel_hr) ? "FreeEngine: composition cancel failed " + Hr(cancel_hr)
                                : "FreeEngine: composition cancelled");
    }
    ClearComposition();
    if (active_context_) { active_context_->Release(); active_context_ = nullptr; }
    if (candidate_window_) candidate_window_->Hide();
    if (runtime_) { hs_runtime_free(runtime_); runtime_ = nullptr; }
    pending_.clear();
    candidates_.clear();
    page_index_ = 0;
    total_candidates_ = 0;
    has_selected_key_ = false;
}

HRESULT HeshunTextService::UpdateComposition(ITfContext* context) {
    if (!context || !runtime_) return E_INVALIDARG;
    const std::wstring text = pending_;
    auto* edit = new (std::nothrow) CompositionEditSession(this, context,
        text.empty() ? CompositionEditSession::Action::Cancel : CompositionEditSession::Action::Update, text);
    if (!edit) return E_OUTOFMEMORY;
    HRESULT session_hr = E_FAIL;
    // Composition state must be updated before the key callback returns. An
    // async request can remain queued while the host advances or ends the
    // composition, which loses no-candidate input from the preedit.
    const HRESULT hr = context->RequestEditSession(client_id_, edit, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE, &session_hr);
    edit->Release();
    Trace(FAILED(hr) ? "UpdateComposition: async RequestEditSession failed " + Hr(hr) :
          session_hr == TF_S_ASYNC ? "UpdateComposition: async edit session queued" :
          FAILED(session_hr) ? "UpdateComposition: async edit session failed " + Hr(session_hr) :
                               "UpdateComposition: async edit session complete");
    return FAILED(hr) ? hr : S_OK;
}

HRESULT HeshunTextService::CancelComposition(ITfContext* context) {
    if (!context || !composition_) return S_OK;
    auto* edit = new (std::nothrow) CompositionEditSession(this, context, CompositionEditSession::Action::Cancel);
    if (!edit) return E_OUTOFMEMORY;
    HRESULT session_hr = E_FAIL;
    const HRESULT hr = context->RequestEditSession(client_id_, edit, TF_ES_SYNC | TF_ES_READWRITE, &session_hr);
    edit->Release();
    Trace(FAILED(hr) ? "CancelComposition: sync RequestEditSession failed " + Hr(hr) :
          FAILED(session_hr) ? "CancelComposition: sync edit session failed " + Hr(session_hr) :
                               "CancelComposition: sync edit session complete");
    return FAILED(hr) ? hr : session_hr;
}

void HeshunTextService::UpdateCandidateWindow() {
    if (!runtime_ || pending_.empty()) {
        if (candidate_window_) candidate_window_->Hide();
        Trace("CandidateWindow: hidden");
        return;
    }
    std::vector<std::wstring> candidates;
    std::vector<CandidateKey> keys;
    for (const auto& candidate : candidates_) {
        std::wstring display = candidate.word;
        if (!candidate.annotation.empty()) display += L"  [" + candidate.annotation + L"]";
        candidates.push_back(std::move(display));
        keys.push_back(candidate.key);
    }
    if (!candidate_window_) {
        candidate_window_ = std::make_unique<CandidateWindow>();
        candidate_window_->SetCandidateClickHandler([this](CandidateKey key) {
            SelectCandidate(active_context_, key);
        });
    }
    candidate_window_->Show(pending_, std::move(candidates), std::move(keys), page_index_, page_size_, total_candidates_, cursor_);
    Trace("CandidateWindow: shown");
}

void HeshunTextService::ChangeCandidatePage(int direction) {
    DispatchRuntime(8, direction);
}

void HeshunTextService::TraceSelectionKey(WPARAM key) const {
    std::ostringstream out;
    out << "CandidateWindow: select key=" << (key == VK_RETURN ? "Enter" : "Space")
        << " ordinal=" << (has_selected_key_ ? selected_key_.ordinal : 0)
        << " keyboard=" << (candidate_window_ && candidate_window_->keyboard_selection() ? "true" : "false");
    Trace(out.str());
}

void HeshunTextService::SelectCandidate(ITfContext* context, CandidateKey key) {
    if (!context || !runtime_ || !DispatchRuntime(6, 0, key) || last_committed_.empty()) return;
    if (candidate_window_) candidate_window_->Hide();
    CommitText(context, last_committed_.c_str());
    Trace("CandidateWindow: mouse candidate selected");
}

bool HeshunTextService::HasPending() const {
    return !pending_.empty();
}

void HeshunTextService::ToggleAsciiMode(ITfContext* context) {
    ascii_mode_ = !ascii_mode_;
    DispatchRuntime(9);
    CancelComposition(context);
    if (candidate_window_) candidate_window_->Hide();
    Trace(ascii_mode_ ? "Mode: English" : "Mode: Chinese");
}

void HeshunTextService::ToggleInputMethod(ITfContext* context) {
    if (!thread_mgr_) return;
    CancelComposition(context);
    FreeEngine();
    pinyin_mode_ = !pinyin_mode_;
    if (!LoadEngine()) {
        Trace("Input method switch failed: engine reload failed");
        return;
    }
    Trace(pinyin_mode_ ? "Input method: Pinyin" : "Input method: Zhengma");
}

void HeshunTextService::ToggleInputMethodFromLangBar() {
    ToggleInputMethod(active_context_);
}

bool HeshunTextService::IsHandledKey(WPARAM key) const {
    if (key == VK_SHIFT) return true;
    if (key == VK_OEM_3 && (GetKeyState(VK_CONTROL) & 0x8000)) return true;
    // Modifier shortcuts belong to the host application. In particular,
    // Ctrl+V must not be interpreted as the letter 'v' by the input method.
    if ((GetKeyState(VK_CONTROL) & 0x8000) ||
        (GetKeyState(VK_MENU) & 0x8000) ||
        (GetKeyState(VK_LWIN) & 0x8000) ||
        (GetKeyState(VK_RWIN) & 0x8000)) return false;
    if (ascii_mode_) return false;
    if (key >= 'A' && key <= 'Z') return true;
    if (key >= 'a' && key <= 'z') return true;
    if (key == VK_BACK) return HasPending();
    if (key == VK_RETURN || key == VK_SPACE) return HasPending();
    if (key == VK_LEFT || key == VK_RIGHT) return HasPending();
    if (key == VK_ESCAPE || key == VK_PRIOR || key == VK_NEXT || key == VK_UP || key == VK_DOWN) return true;
    return key >= '1' && key <= '9';
}

bool HeshunTextService::FeedKey(WPARAM key, std::string& committed) {
    committed.clear();
    if (!runtime_) return false;
    if (key >= 'A' && key <= 'Z') { const bool consumed = DispatchRuntime(0, static_cast<long long>(key - 'A' + 'a')); committed = last_committed_; return consumed; }
    if (key >= 'a' && key <= 'z') { const bool consumed = DispatchRuntime(0, static_cast<long long>(key)); committed = last_committed_; return consumed; }
    if (key == VK_BACK) return DispatchRuntime(1);
    if (key == VK_ESCAPE) return DispatchRuntime(3);
    if (key == VK_LEFT || key == VK_RIGHT) {
        DispatchRuntime(13, key == VK_RIGHT ? 1 : -1);
        UpdateComposition(active_context_);
        UpdateCandidateWindow();
        return true;
    }
    if (key == VK_PRIOR || key == VK_NEXT) { ChangeCandidatePage(key == VK_NEXT ? 1 : -1); return true; }
    if (key == VK_UP || key == VK_DOWN) {
        DispatchRuntime(7, key == VK_DOWN ? 1 : -1);
        if (candidate_window_) { candidate_window_->MoveSelection(key == VK_DOWN ? 1 : -1); candidate_window_->UseKeyboardSelection(); }
        Trace(key == VK_DOWN ? "CandidateWindow: selection down" : "CandidateWindow: selection up");
        return true;
    }
    if (key == VK_RETURN || key == VK_SPACE) {
        TraceSelectionKey(key);
        DispatchRuntime(key == VK_RETURN ? 5 : 4);
        committed = last_committed_;
        if (committed.empty() && !pending_.empty()) {
            // 没有候选时，Space/Enter 按输入法收尾语义提交当前预编辑原文。
            const int needed = WideCharToMultiByte(CP_UTF8, 0, pending_.data(), static_cast<int>(pending_.size()), nullptr, 0, nullptr, nullptr);
            if (needed > 0) {
                std::string utf8(static_cast<size_t>(needed), '\0');
                if (WideCharToMultiByte(CP_UTF8, 0, pending_.data(), static_cast<int>(pending_.size()), utf8.data(), needed, nullptr, nullptr)) {
                    committed = std::move(utf8);
                }
            }
            DispatchRuntime(3);
        }
        if (!committed.empty()) Trace(std::string("CandidateWindow: selected text=") + committed);
        return true;
    }
    if (key >= '1' && key <= '9') {
        const size_t row = static_cast<size_t>(key - '1');
        if (row < candidates_.size()) DispatchRuntime(6, 0, candidates_[row].key);
        committed = last_committed_;
        return true;
    }
    return false;
}

STDMETHODIMP HeshunTextService::OnSetFocus(BOOL foreground) {
    if (foreground) return S_OK;

    // A TSF context can disappear while the service still owns its last
    // reference. End the composition and reset the owned runtime before the
    // old context is released; otherwise a later host can inherit stale
    // preedit/candidates or receive a commit targeted at the wrong document.
    if (active_context_) {
        const HRESULT cancel_hr = CancelComposition(active_context_);
        Trace(FAILED(cancel_hr) ? "FocusLost: composition cancel failed " + Hr(cancel_hr)
                                : "FocusLost: composition cancelled");
        DispatchRuntime(12);
        if (candidate_window_) candidate_window_->Hide();
        active_context_->Release();
        active_context_ = nullptr;
    }
    return S_OK;
}
STDMETHODIMP HeshunTextService::OnTestKeyDown(ITfContext*, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = IsHandledKey(wparam) ? TRUE : FALSE;
    if (*eaten) Trace("OnTestKeyDown: handled");
    return S_OK;
}

STDMETHODIMP HeshunTextService::OnKeyDown(ITfContext* context, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    if (context && context != active_context_) {
        if (active_context_) {
            const HRESULT cancel_hr = CancelComposition(active_context_);
            Trace(FAILED(cancel_hr) ? "ContextChanged: composition cancel failed " + Hr(cancel_hr)
                                    : "ContextChanged: composition cancelled");
            DispatchRuntime(12);
            if (candidate_window_) candidate_window_->Hide();
            active_context_->Release();
            active_context_ = nullptr;
        }
        active_context_ = context;
        active_context_->AddRef();
        Trace("ContextChanged: active context replaced");
    }
    *eaten = FALSE;
    if (wparam == VK_SHIFT) {
        shift_down_ = true;
        shift_used_with_other_key_ = false;
        *eaten = TRUE;
        return S_OK;
    }
    if (wparam == VK_OEM_3 && (GetKeyState(VK_CONTROL) & 0x8000)) {
        *eaten = TRUE;
        ToggleInputMethod(context);
        return S_OK;
    }
    if (shift_down_) shift_used_with_other_key_ = true;
    if (!IsHandledKey(wparam)) return S_OK;
    Trace("OnKeyDown: handled");
    std::string committed;
    if (!FeedKey(wparam, committed)) {
        // The TSF already owns this key in Chinese mode. The engine may reject
        // an invalid continuation while preserving the previous composition;
        // never let the rejected letter fall through to the host application.
        *eaten = TRUE;
        UpdateComposition(context);
        UpdateCandidateWindow();
        Trace("OnKeyDown: engine rejected key; key eaten");
        return S_OK;
    }
    *eaten = TRUE;
    if (!committed.empty()) {
        if (candidate_window_) candidate_window_->Hide();
        Trace("OnKeyDown: committing engine result");
        const HRESULT hr = CommitText(context, committed.c_str());
        Trace(FAILED(hr) ? "OnKeyDown: CommitText failed" : "OnKeyDown: CommitText succeeded");
        return hr;
    }
    UpdateComposition(context);
    UpdateCandidateWindow();
    Trace("OnKeyDown: awaiting more input");
    return S_OK;
}

STDMETHODIMP HeshunTextService::OnTestKeyUp(ITfContext*, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = wparam == VK_SHIFT ? TRUE : FALSE;
    return S_OK;
}
STDMETHODIMP HeshunTextService::OnKeyUp(ITfContext* context, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = FALSE;
    if (wparam == VK_SHIFT) {
        const bool toggle = shift_down_ && !shift_used_with_other_key_;
        shift_down_ = false;
        shift_used_with_other_key_ = false;
        if (toggle) ToggleAsciiMode(context);
        *eaten = TRUE;
    }
    return S_OK;
}
STDMETHODIMP HeshunTextService::OnPreservedKey(ITfContext*, REFGUID, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = FALSE;
    return S_OK;
}

STDMETHODIMP HeshunTextService::EnumDisplayAttributeInfo(IEnumTfDisplayAttributeInfo** result) {
    Trace("DisplayAttribute: EnumDisplayAttributeInfo");
    return CreateHeshunDisplayAttributeEnum(result);
}

STDMETHODIMP HeshunTextService::GetDisplayAttributeInfo(REFGUID guid, ITfDisplayAttributeInfo** result) {
    Trace("DisplayAttribute: GetDisplayAttributeInfo");
    if (!result) return E_INVALIDARG;
    *result = nullptr;
    if (guid != GUID_DISPLAYATTRIBUTE_HESHUN_PREEDIT) return E_INVALIDARG;
    auto* info = new (std::nothrow) HeshunDisplayAttributeInfo();
    if (!info) return E_OUTOFMEMORY;
    *result = info;
    return S_OK;
}

HRESULT HeshunTextService::CommitText(ITfContext* context, const char* utf8) {
    if (!context) return E_INVALIDARG;
    std::wstring text = Utf8ToUtf16(utf8);
    if (text.empty()) return S_OK;
    if (composition_) {
        auto* edit = new (std::nothrow) CompositionEditSession(this, context,
            CompositionEditSession::Action::Commit, std::move(text));
        if (!edit) return E_OUTOFMEMORY;
        HRESULT session_hr = E_FAIL;
        const HRESULT hr = context->RequestEditSession(client_id_, edit, TF_ES_ASYNC | TF_ES_READWRITE, &session_hr);
        edit->Release();
        Trace(FAILED(hr) ? "CommitText: composition RequestEditSession failed " + Hr(hr) :
              session_hr == TF_S_ASYNC ? "CommitText: composition edit session queued" :
              FAILED(session_hr) ? "CommitText: composition edit session failed " + Hr(session_hr) :
                                   "CommitText: composition edit session complete");
        return FAILED(hr) ? hr : S_OK;
    }
    auto* edit = new (std::nothrow) CommitEditSession(context, std::move(text));
    if (!edit) return E_OUTOFMEMORY;
    // A key callback runs while TSF may hold the document lock. Use an
    // asynchronous edit session instead of synchronously re-entering the host.
    HRESULT session_hr = E_FAIL;
    Trace("CommitText: requesting asynchronous edit session");
    const HRESULT hr = context->RequestEditSession(client_id_, edit, TF_ES_ASYNC | TF_ES_READWRITE, &session_hr);
    edit->Release();
    Trace(FAILED(hr) ? "CommitText: RequestEditSession failed " + Hr(hr) :
          session_hr == TF_S_ASYNC ? "CommitText: edit session queued" :
          FAILED(session_hr) ? "CommitText: edit session failed " + Hr(session_hr) : "CommitText: complete");
    return FAILED(hr) ? hr : S_OK;
}

HRESULT CreateHeshunTextService(REFIID riid, void** object) {
    auto* service = new (std::nothrow) HeshunTextService();
    if (!service) return E_OUTOFMEMORY;
    const HRESULT hr = service->QueryInterface(riid, object);
    service->Release();
    return hr;
}

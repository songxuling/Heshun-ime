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

#include "guids.h"
#include "text_service.h"

extern HINSTANCE g_module_instance;
extern volatile LONG g_object_count;

namespace {

void Trace(const std::string& message);
std::string Hr(HRESULT hr);

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
        if (action_ == Action::Cancel) return End(ec, true);
        if (action_ == Action::Commit) return Commit(ec);
        return Update(ec);
    }

private:
    HRESULT Update(TfEditCookie ec) {
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

    ITfKeystrokeMgr* keystrokes = nullptr;
    HRESULT hr = thread_mgr_->QueryInterface(IID_PPV_ARGS(&keystrokes));
    if (SUCCEEDED(hr)) {
        hr = keystrokes->AdviseKeyEventSink(client_id_, static_cast<ITfKeyEventSink*>(this), TRUE);
        keystrokes->Release();
    }
    if (FAILED(hr)) { Trace("ActivateEx: AdviseKeyEventSink failed"); Deactivate(); }
    else Trace("ActivateEx: key sink advised");
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
    if (directory.empty()) { Trace("LoadEngine: module directory unavailable"); return false; }
    const std::string schema = directory + "\\schemas\\zhengma66.schema.yaml";
    Trace("LoadEngine schema: " + schema);
    engine_ = hs_engine_load_schema(schema.c_str());
    if (!engine_) { Trace("LoadEngine: hs_engine_load_schema returned null"); return false; }
    session_ = hs_session_new(engine_);
    if (!session_) Trace("LoadEngine: hs_session_new returned null");
    if (session_) hs_set_ascii_mode(session_, ascii_mode_ ? 1 : 0);
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
    ClearComposition();
    if (active_context_) { active_context_->Release(); active_context_ = nullptr; }
    if (candidate_window_) candidate_window_->Hide();
    if (session_) { hs_session_free(session_); session_ = nullptr; }
    if (engine_) { hs_engine_free(engine_); engine_ = nullptr; }
}

HRESULT HeshunTextService::UpdateComposition(ITfContext* context) {
    if (!context || !session_) return E_INVALIDARG;
    char* pending = hs_pending(session_);
    const std::wstring text = pending ? Utf8ToUtf16(pending) : L"";
    if (pending) hs_str_free(pending);
    auto* edit = new (std::nothrow) CompositionEditSession(this, context,
        text.empty() ? CompositionEditSession::Action::Cancel : CompositionEditSession::Action::Update, text);
    if (!edit) return E_OUTOFMEMORY;
    HRESULT session_hr = E_FAIL;
    const HRESULT hr = context->RequestEditSession(client_id_, edit, TF_ES_ASYNC | TF_ES_READWRITE, &session_hr);
    edit->Release();
    return FAILED(hr) ? hr : S_OK;
}

HRESULT HeshunTextService::CancelComposition(ITfContext* context) {
    if (!context || !composition_) return S_OK;
    auto* edit = new (std::nothrow) CompositionEditSession(this, context, CompositionEditSession::Action::Cancel);
    if (!edit) return E_OUTOFMEMORY;
    HRESULT session_hr = E_FAIL;
    const HRESULT hr = context->RequestEditSession(client_id_, edit, TF_ES_ASYNC | TF_ES_READWRITE, &session_hr);
    edit->Release();
    return FAILED(hr) ? hr : S_OK;
}

void HeshunTextService::UpdateCandidateWindow() {
    if (!session_) return;
    char* pending_raw = hs_pending(session_);
    char* candidates_raw = hs_candidates_page(session_, candidate_offset_, 9);
    std::wstring pending = pending_raw ? Utf8ToUtf16(pending_raw) : L"";
    std::vector<std::wstring> candidates = ParseCandidateWords(candidates_raw);
    if (pending_raw) hs_str_free(pending_raw);
    if (candidates_raw) hs_str_free(candidates_raw);

    if (pending.empty() || candidates.empty()) {
        if (candidate_window_) candidate_window_->Hide();
        Trace("CandidateWindow: hidden");
        return;
    }
    if (!candidate_window_) {
        candidate_window_ = std::make_unique<CandidateWindow>();
        candidate_window_->SetCandidateClickHandler([this](size_t index) {
            SelectCandidate(active_context_, index);
        });
    }
    candidate_window_->Show(std::move(pending), std::move(candidates));
    Trace("CandidateWindow: shown");
}

bool HeshunTextService::HasCandidatePage(int offset) const {
    if (!session_ || offset < 0) return false;
    char* page = hs_candidates_page(session_, offset, 1);
    const bool exists = page && *page;
    if (page) hs_str_free(page);
    return exists;
}

void HeshunTextService::ChangeCandidatePage(int direction) {
    if (!session_ || !HasPending()) return;
    const int next = std::max(0, candidate_offset_ + direction * 9);
    if (next != candidate_offset_ && !HasCandidatePage(next)) {
        Trace(direction > 0 ? "CandidateWindow: next page unavailable" : "CandidateWindow: already first page");
        return;
    }
    candidate_offset_ = next;
    UpdateCandidateWindow();
    std::ostringstream out;
    out << (direction > 0 ? "CandidateWindow: next page offset=" : "CandidateWindow: previous page offset=") << candidate_offset_;
    Trace(out.str());
}

int HeshunTextService::SelectedCandidateIndex() const {
    const size_t row = candidate_window_ ? candidate_window_->selected_index() : 0;
    return candidate_offset_ + static_cast<int>(row) + 1;
}

void HeshunTextService::TraceSelectionKey(WPARAM key) const {
    std::ostringstream out;
    out << "CandidateWindow: select key=" << (key == VK_RETURN ? "Enter" : "Space")
        << " index=" << SelectedCandidateIndex()
        << " keyboard=" << (candidate_window_ && candidate_window_->keyboard_selection() ? "true" : "false");
    Trace(out.str());
}

void HeshunTextService::SelectCandidate(ITfContext* context, size_t index) {
    if (!context || !session_) return;
    const int candidate_index = candidate_offset_ + static_cast<int>(index) + 1;
    char* committed = hs_select(session_, candidate_index);
    if (!committed) return;
    if (candidate_window_) candidate_window_->Hide();
    CommitText(context, committed);
    hs_str_free(committed);
    Trace("CandidateWindow: mouse candidate selected");
}

bool HeshunTextService::HasPending() const {
    if (!session_) return false;
    char* pending = hs_pending(session_);
    const bool has_pending = pending && *pending;
    if (pending) hs_str_free(pending);
    return has_pending;
}

void HeshunTextService::ToggleAsciiMode(ITfContext* context) {
    ascii_mode_ = !ascii_mode_;
    if (session_) {
        hs_clear(session_);
        hs_set_ascii_mode(session_, ascii_mode_ ? 1 : 0);
    }
    CancelComposition(context);
    if (candidate_window_) candidate_window_->Hide();
    Trace(ascii_mode_ ? "Mode: English" : "Mode: Chinese");
}

bool HeshunTextService::IsHandledKey(WPARAM key) const {
    if (key == VK_SHIFT) return true;
    if (ascii_mode_) return false;
    if (key >= 'A' && key <= 'Z') return true;
    if (key >= 'a' && key <= 'z') return true;
    if (key == VK_BACK) return HasPending();
    if (key == VK_ESCAPE || key == VK_SPACE || key == VK_PRIOR || key == VK_NEXT || key == VK_UP || key == VK_DOWN || key == VK_RETURN) return true;
    return key >= '1' && key <= '9';
}

bool HeshunTextService::FeedKey(WPARAM key, char** committed) {
    if (!session_) return false;
    *committed = nullptr;
    if (key >= 'A' && key <= 'Z') { candidate_offset_ = 0; return hs_feed(session_, static_cast<char>(key - 'A' + 'a'), committed) != 0; }
    if (key >= 'a' && key <= 'z') { candidate_offset_ = 0; return hs_feed(session_, static_cast<char>(key), committed) != 0; }
    if (key == VK_BACK) { hs_backspace(session_); candidate_offset_ = 0; return true; }
    if (key == VK_ESCAPE) { hs_clear(session_); candidate_offset_ = 0; return true; }
    if (key == VK_PRIOR || key == VK_NEXT) { ChangeCandidatePage(key == VK_NEXT ? 1 : -1); return true; }
    if (key == VK_UP || key == VK_DOWN) {
        if (candidate_window_) {
            candidate_window_->MoveSelection(key == VK_DOWN ? 1 : -1);
            candidate_window_->UseKeyboardSelection();
        }
        Trace(key == VK_DOWN ? "CandidateWindow: selection down" : "CandidateWindow: selection up");
        return true;
    }
    if (key == VK_RETURN || key == VK_SPACE) {
        TraceSelectionKey(key);
        *committed = hs_select(session_, SelectedCandidateIndex());
        if (*committed) Trace(std::string("CandidateWindow: selected text=") + *committed);
        candidate_offset_ = 0;
        return true;
    }
    if (key >= '1' && key <= '9') { *committed = hs_select(session_, candidate_offset_ + static_cast<int>(key - '0')); candidate_offset_ = 0; return true; }
    return false;
}

STDMETHODIMP HeshunTextService::OnSetFocus(BOOL) { return S_OK; }
STDMETHODIMP HeshunTextService::OnTestKeyDown(ITfContext*, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    *eaten = IsHandledKey(wparam) ? TRUE : FALSE;
    if (*eaten) Trace("OnTestKeyDown: handled");
    return S_OK;
}

STDMETHODIMP HeshunTextService::OnKeyDown(ITfContext* context, WPARAM wparam, LPARAM, BOOL* eaten) {
    if (!eaten) return E_INVALIDARG;
    if (context && context != active_context_) {
        if (active_context_) active_context_->Release();
        active_context_ = context;
        active_context_->AddRef();
    }
    *eaten = FALSE;
    if (wparam == VK_SHIFT) {
        shift_down_ = true;
        shift_used_with_other_key_ = false;
        *eaten = TRUE;
        return S_OK;
    }
    if (shift_down_) shift_used_with_other_key_ = true;
    if (!IsHandledKey(wparam)) return S_OK;
    Trace("OnKeyDown: handled");
    char* committed = nullptr;
    if (!FeedKey(wparam, &committed)) { Trace("OnKeyDown: engine rejected key"); return S_OK; }
    *eaten = TRUE;
    if (committed) {
        if (candidate_window_) candidate_window_->Hide();
        Trace("OnKeyDown: committing engine result");
        const HRESULT hr = CommitText(context, committed);
        hs_str_free(committed);
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

#pragma once

#include <windows.h>
#include <msctf.h>
#include <memory>
#include "heshun.h"
#include "candidate_window.h"

class HeshunTextService final : public ITfTextInputProcessorEx, public ITfKeyEventSink {
public:
    HeshunTextService();

    // IUnknown
    STDMETHODIMP QueryInterface(REFIID riid, void** object) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;

    // ITfTextInputProcessor
    STDMETHODIMP Activate(ITfThreadMgr* thread_mgr, TfClientId client_id) override;
    STDMETHODIMP Deactivate() override;

    // ITfTextInputProcessorEx
    STDMETHODIMP ActivateEx(ITfThreadMgr* thread_mgr, TfClientId client_id, DWORD flags) override;

    // ITfKeyEventSink
    STDMETHODIMP OnSetFocus(BOOL foreground) override;
    STDMETHODIMP OnTestKeyDown(ITfContext* context, WPARAM wparam, LPARAM lparam, BOOL* eaten) override;
    STDMETHODIMP OnKeyDown(ITfContext* context, WPARAM wparam, LPARAM lparam, BOOL* eaten) override;
    STDMETHODIMP OnTestKeyUp(ITfContext* context, WPARAM wparam, LPARAM lparam, BOOL* eaten) override;
    STDMETHODIMP OnKeyUp(ITfContext* context, WPARAM wparam, LPARAM lparam, BOOL* eaten) override;
    STDMETHODIMP OnPreservedKey(ITfContext* context, REFGUID guid, BOOL* eaten) override;

    ITfComposition* composition() const { return composition_; }
    void SetComposition(ITfComposition* composition);
    void ClearComposition();

private:
    ~HeshunTextService();

    bool LoadEngine();
    const char* ActiveSchemaId() const;
    const char* ActiveSchemaFile() const;
    const char* ActiveUserDictFile() const;
    void FreeEngine();
    bool HasPending() const;
    bool IsHandledKey(WPARAM key) const;
    void ToggleAsciiMode(ITfContext* context);
    void ToggleInputMethod(ITfContext* context);
    void SelectCandidate(ITfContext* context, size_t index);
    void ChangeCandidatePage(int direction);
    bool HasCandidatePage(int offset) const;
    int SelectedCandidateIndex() const;
    void TraceSelectionKey(WPARAM key) const;
    bool FeedKey(WPARAM key, char** committed);
    HRESULT CommitText(ITfContext* context, const char* utf8);
    HRESULT UpdateComposition(ITfContext* context);
    HRESULT CancelComposition(ITfContext* context);
    void UpdateCandidateWindow();
    void SaveUserDictionary();

    volatile LONG ref_count_ = 1;
    ITfThreadMgr* thread_mgr_ = nullptr;
    TfClientId client_id_ = TF_CLIENTID_NULL;
    DWORD key_sink_cookie_ = TF_INVALID_COOKIE;
    hs_handle* engine_ = nullptr;
    hs_handle* session_ = nullptr;
    ITfComposition* composition_ = nullptr;
    std::unique_ptr<CandidateWindow> candidate_window_;
    ITfContext* active_context_ = nullptr;
    bool ascii_mode_ = false;
    bool pinyin_mode_ = false;
    bool shift_down_ = false;
    bool shift_used_with_other_key_ = false;
    int candidate_offset_ = 0;
};

HRESULT CreateHeshunTextService(REFIID riid, void** object);

#pragma once

#include <windows.h>
#include <msctf.h>
#include "heshun.h"

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

private:
    ~HeshunTextService();

    bool LoadEngine();
    void FreeEngine();
    bool IsHandledKey(WPARAM key) const;
    bool FeedKey(WPARAM key, char** committed);
    HRESULT CommitText(ITfContext* context, const char* utf8);
    void SaveUserDictionary();

    volatile LONG ref_count_ = 1;
    ITfThreadMgr* thread_mgr_ = nullptr;
    TfClientId client_id_ = TF_CLIENTID_NULL;
    DWORD key_sink_cookie_ = TF_INVALID_COOKIE;
    hs_handle* engine_ = nullptr;
    hs_handle* session_ = nullptr;
};

HRESULT CreateHeshunTextService(REFIID riid, void** object);

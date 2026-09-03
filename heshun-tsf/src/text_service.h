#pragma once

#include <windows.h>
#include <msctf.h>
#include <memory>
#include <string>
#include "heshun.h"
#include "candidate_window.h"
#include "display_attributes.h"
#include "compartment_sink.h"
#include "candidate_list.h"

struct IHeshunLangBarStatus {
    virtual void NotifyUpdate(DWORD flags = TF_LBI_TEXT | TF_LBI_ICON | TF_LBI_STATUS) = 0;
};

class HeshunTextService final : public ITfTextInputProcessorEx,
                                public ITfKeyEventSink,
                                public ITfThreadMgrEventSink,
                                public ITfTextEditSink,
                                public ITfTextLayoutSink,
                                public ITfThreadFocusSink,
                                public ITfActiveLanguageProfileNotifySink,
                                public ITfDisplayAttributeProvider,
                                public ITfCompositionSink {
public:
    HeshunTextService();
    void ToggleInputMethodFromLangBar();
    void SelectInputMethodFromLangBar(bool pinyin);
    HWND FocusedContextWindow() const;
    bool IsPinyinMode() const { return pinyin_mode_; }
    bool ascii_mode() const { return ascii_mode_; }

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

    // ITfThreadMgrEventSink
    STDMETHODIMP OnInitDocumentMgr(ITfDocumentMgr* document_manager) override;
    STDMETHODIMP OnUninitDocumentMgr(ITfDocumentMgr* document_manager) override;
    STDMETHODIMP OnSetFocus(ITfDocumentMgr* focused_document_manager,
                            ITfDocumentMgr* previous_document_manager) override;
    STDMETHODIMP OnPushContext(ITfContext* context) override;
    STDMETHODIMP OnPopContext(ITfContext* context) override;

    // ITfTextEditSink
    STDMETHODIMP OnEndEdit(ITfContext* context,
                           TfEditCookie edit_cookie,
                           ITfEditRecord* edit_record) override;

    // ITfTextLayoutSink
    STDMETHODIMP OnLayoutChange(ITfContext* context,
                                TfLayoutCode layout_code,
                                ITfContextView* context_view) override;

    // ITfThreadFocusSink
    STDMETHODIMP OnSetThreadFocus() override;
    STDMETHODIMP OnKillThreadFocus() override;

    // ITfActiveLanguageProfileNotifySink
    STDMETHODIMP OnActivated(REFCLSID clsid, REFGUID profile, BOOL activated) override;

    // ITfDisplayAttributeProvider
    STDMETHODIMP EnumDisplayAttributeInfo(IEnumTfDisplayAttributeInfo** result) override;
    STDMETHODIMP GetDisplayAttributeInfo(REFGUID guid, ITfDisplayAttributeInfo** result) override;

    // ITfCompositionSink
    STDMETHODIMP OnCompositionTerminated(TfEditCookie ec_write, ITfComposition* composition) override;

    ITfComposition* composition() const { return composition_; }
    bool composition_end_in_progress() const { return composition_end_in_progress_; }
    void set_composition_end_in_progress(bool value) { composition_end_in_progress_ = value; }
    TfGuidAtom display_attribute_atom() const { return display_attribute_atom_; }
    void SetComposition(ITfComposition* composition);
    void ClearComposition();
    void QueueCompositionUpdate(ITfContext* context);
    void UpdateCandidateAnchor(const RECT& rect);
    void OnCompartmentChanged(REFGUID guid);
    HRESULT FocusedDocumentManager(ITfDocumentMgr** manager) const;
    size_t candidate_count() const { return candidates_.size(); }
    unsigned int candidate_page() const { return page_index_; }
    unsigned int candidate_page_count() const { return page_size_ ? (total_candidates_ + page_size_ - 1) / page_size_ : 0; }
    size_t selected_candidate_index() const;
    unsigned int composition_cursor() const { return cursor_; }
    HRESULT CandidateString(size_t index, BSTR* string) const;
    void SetCandidatePage(unsigned int page);
    void HighlightCandidate(size_t index);
    HRESULT FinalizeCandidate(size_t index);
    HRESULT AbortCandidate();
    HRESULT QueryUIElementMgr(ITfUIElementMgr** manager) const;

private:
    friend class HeshunCandidateList;
    ~HeshunTextService();

    bool LoadEngine();
    void RefreshPersistentInputMode();
    const char* ActiveSchemaId() const;
    const char* ActiveSchemaFile() const;
    const char* ActiveUserDictFile() const;
    void FreeEngine();
    bool HasPending() const;
    bool IsHandledKey(WPARAM key) const;
    bool IsCursorKey(WPARAM key) const;
    void ToggleAsciiMode(ITfContext* context);
    void ToggleInputMethod(ITfContext* context);
    void SelectCandidate(ITfContext* context, CandidateKey key);
    void ChangeCandidatePage(int direction);
    bool DispatchRuntime(unsigned int opcode, long long value = 0, CandidateKey key = {});
    void TraceSelectionKey(WPARAM key) const;
    bool FeedKey(WPARAM key, std::string& committed);
    HRESULT CommitText(ITfContext* context, const char* utf8);
    HRESULT UpdateComposition(ITfContext* context);
    HRESULT CancelComposition(ITfContext* context);
    void UpdateCandidateWindow();
    void SaveUserDictionary();
    HRESULT SetCompartmentDWORD(REFGUID guid, DWORD value);
    void SyncKeyboardCompartments();
    HRESULT InitActiveLanguageProfileNotifySink();
    void UninitActiveLanguageProfileNotifySink();
    HRESULT InitTextEditSink(ITfDocumentMgr* document_manager);
    void UninitTextEditSink();
    HRESULT InitTextLayoutSink(ITfDocumentMgr* document_manager);
    void UninitTextLayoutSink();
    HRESULT InitThreadFocusSink();
    void UninitThreadFocusSink();
    HRESULT InitCompartmentSinks();
    void UninitCompartmentSinks();
    bool ReadCompartmentDWORD(REFGUID guid, DWORD* value) const;
    void ShowLanguageBar(bool show);
    void ClearActiveContext(const char* reason);

    volatile LONG ref_count_ = 1;
    ITfThreadMgr* thread_mgr_ = nullptr;
    TfClientId client_id_ = TF_CLIENTID_NULL;
    DWORD key_sink_cookie_ = TF_INVALID_COOKIE;
    DWORD thread_mgr_event_sink_cookie_ = TF_INVALID_COOKIE;
    DWORD active_profile_sink_cookie_ = TF_INVALID_COOKIE;
    DWORD text_edit_sink_cookie_ = TF_INVALID_COOKIE;
    DWORD text_layout_sink_cookie_ = TF_INVALID_COOKIE;
    DWORD thread_focus_sink_cookie_ = TF_INVALID_COOKIE;
    std::vector<std::unique_ptr<class HeshunCompartmentSink>> compartment_sinks_;
    hs_handle* runtime_ = nullptr;
    ITfComposition* composition_ = nullptr;
    bool composition_end_in_progress_ = false;
    ITfLangBarItemMgr* langbar_mgr_ = nullptr;
    ITfLangBarItem* langbar_item_ = nullptr;
    IHeshunLangBarStatus* langbar_status_ = nullptr;
    std::unique_ptr<CandidateWindow> candidate_window_;
    std::unique_ptr<HeshunCandidateList> candidate_list_;
    ITfContext* active_context_ = nullptr;
    ITfContext* text_edit_sink_context_ = nullptr;
    ITfContext* text_layout_sink_context_ = nullptr;
    bool ascii_mode_ = false;
    bool keyboard_disabled_ = false;
    bool empty_context_ = false;
    bool keyboard_open_ = true;
    DWORD conversion_mode_ = TF_CONVERSIONMODE_NATIVE;
    bool pinyin_mode_ = false;
    bool shift_down_ = false;
    bool shift_used_with_other_key_ = false;
    bool test_key_down_pending_ = false;
    bool test_key_up_pending_ = false;
    struct RuntimeCandidate {
        CandidateKey key;
        std::wstring word;
        std::wstring annotation;
        std::wstring label;
    };
    std::wstring pending_;
    std::vector<RuntimeCandidate> candidates_;
    unsigned int page_index_ = 0;
    unsigned int page_size_ = 9;
    unsigned int total_candidates_ = 0;
    unsigned int cursor_ = 0;
    CandidateKey selected_key_{};
    bool has_selected_key_ = false;
    bool has_previous_page_ = false;
    bool has_next_page_ = false;
    std::string last_committed_;
    TfGuidAtom display_attribute_atom_ = TF_INVALID_GUIDATOM;
};

HRESULT CreateHeshunTextService(REFIID riid, void** object);

#pragma once
#include <windows.h>
#include <msctf.h>
#include "ctffunc.h"

class HeshunTextService;

class HeshunCandidateList final : public ITfIntegratableCandidateListUIElement,
                                  public ITfCandidateListUIElementBehavior {
public:
    explicit HeshunCandidateList(HeshunTextService* service);
    ~HeshunCandidateList();
    STDMETHODIMP QueryInterface(REFIID riid, void** object) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;
    STDMETHODIMP GetDescription(BSTR* description) override;
    STDMETHODIMP GetGUID(GUID* guid) override;
    STDMETHODIMP Show(BOOL show) override;
    STDMETHODIMP IsShown(BOOL* shown) override;
    STDMETHODIMP GetUpdatedFlags(DWORD* flags) override;
    STDMETHODIMP GetDocumentMgr(ITfDocumentMgr** manager) override;
    STDMETHODIMP GetCount(UINT* count) override;
    STDMETHODIMP GetSelection(UINT* selection) override;
    STDMETHODIMP GetString(UINT index, BSTR* string) override;
    STDMETHODIMP GetPageIndex(UINT* index, UINT size, UINT* page_count) override;
    STDMETHODIMP SetPageIndex(UINT* index, UINT page_count) override;
    STDMETHODIMP GetCurrentPage(UINT* page) override;
    STDMETHODIMP SetSelection(UINT index) override;
    STDMETHODIMP Finalize() override;
    STDMETHODIMP Abort() override;
    STDMETHODIMP SetIntegrationStyle(GUID style) override;
    STDMETHODIMP GetSelectionStyle(TfIntegratableCandidateListSelectionStyle* style) override;
    STDMETHODIMP OnKeyDown(WPARAM wparam, LPARAM lparam, BOOL* eaten) override;
    STDMETHODIMP ShowCandidateNumbers(BOOL* show) override;
    STDMETHODIMP FinalizeExactCompositionString() override;
    HRESULT Begin();
    void End();
    void Update();
    UINT ui_id() const { return ui_id_; }
private:
    LONG ref_count_ = 1;
    HeshunTextService* service_ = nullptr;
    DWORD ui_id_ = 0;
    bool shown_ = false;
    UINT selected_ = 0;
    TfIntegratableCandidateListSelectionStyle style_ = STYLE_ACTIVE_SELECTION;
};

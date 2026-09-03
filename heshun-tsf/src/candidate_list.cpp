#include "candidate_list.h"
#include "text_service.h"
#include "guids.h"

HeshunCandidateList::HeshunCandidateList(HeshunTextService* service) : service_(service) {
    if (service_) service_->AddRef();
}
HeshunCandidateList::~HeshunCandidateList() { End(); if (service_) service_->Release(); }
STDMETHODIMP HeshunCandidateList::QueryInterface(REFIID riid, void** object) {
    if (!object) return E_INVALIDARG;
    *object = nullptr;
    if (riid == IID_IUnknown || riid == __uuidof(ITfIntegratableCandidateListUIElement))
        *object = static_cast<ITfIntegratableCandidateListUIElement*>(this);
    else if (riid == IID_ITfUIElement || riid == IID_ITfCandidateListUIElement || riid == IID_ITfCandidateListUIElementBehavior)
        *object = static_cast<ITfCandidateListUIElementBehavior*>(this);
    if (!*object) return E_NOINTERFACE;
    AddRef(); return S_OK;
}
STDMETHODIMP_(ULONG) HeshunCandidateList::AddRef() { return InterlockedIncrement(&ref_count_); }
STDMETHODIMP_(ULONG) HeshunCandidateList::Release() { ULONG n=InterlockedDecrement(&ref_count_); if(!n) delete this; return n; }
STDMETHODIMP HeshunCandidateList::GetDescription(BSTR* d) { if(!d)return E_INVALIDARG; *d=SysAllocString(L"heshun Candidate List"); return *d?S_OK:E_OUTOFMEMORY; }
STDMETHODIMP HeshunCandidateList::GetGUID(GUID* g) { if(!g)return E_INVALIDARG; *g=GUID_LBI_INPUTMODE_HESHUN; return S_OK; }
STDMETHODIMP HeshunCandidateList::Show(BOOL show) { shown_=show!=FALSE; return S_OK; }
STDMETHODIMP HeshunCandidateList::IsShown(BOOL* shown) { if(!shown)return E_INVALIDARG; *shown = shown_?TRUE:FALSE; return S_OK; }
STDMETHODIMP HeshunCandidateList::GetUpdatedFlags(DWORD* flags) { if(!flags)return E_INVALIDARG; *flags=TF_CLUIE_DOCUMENTMGR|TF_CLUIE_COUNT|TF_CLUIE_SELECTION|TF_CLUIE_STRING|TF_CLUIE_PAGEINDEX|TF_CLUIE_CURRENTPAGE; return S_OK; }
STDMETHODIMP HeshunCandidateList::GetDocumentMgr(ITfDocumentMgr** manager) { if(!manager)return E_INVALIDARG; *manager=nullptr; return service_?service_->FocusedDocumentManager(manager):E_FAIL; }
STDMETHODIMP HeshunCandidateList::GetCount(UINT* count) { if(!count)return E_INVALIDARG; *count=service_?static_cast<UINT>(service_->candidate_count()):0; return S_OK; }
STDMETHODIMP HeshunCandidateList::GetSelection(UINT* selection) { if(!selection)return E_INVALIDARG; selected_=service_?static_cast<UINT>(service_->selected_candidate_index()):0; *selection=selected_; return S_OK; }
STDMETHODIMP HeshunCandidateList::GetString(UINT index, BSTR* string) { if(!string)return E_INVALIDARG; *string=nullptr; return service_?service_->CandidateString(index,string):E_FAIL; }
STDMETHODIMP HeshunCandidateList::GetPageIndex(UINT* index, UINT size, UINT* count) { if(!count)return E_INVALIDARG; UINT n=service_?service_->candidate_page_count():0; *count=n; if(index){if(size<n)return E_INVALIDARG; for(UINT i=0;i<n;++i)index[i]=i;} return S_OK; }
STDMETHODIMP HeshunCandidateList::SetPageIndex(UINT* index, UINT page_count) { if(!index||!service_||*index>=page_count)return E_INVALIDARG; service_->SetCandidatePage(*index); return S_OK; }
STDMETHODIMP HeshunCandidateList::GetCurrentPage(UINT* page) { if(!page)return E_INVALIDARG; *page=service_?service_->candidate_page():0; return S_OK; }
STDMETHODIMP HeshunCandidateList::SetSelection(UINT index) { if(!service_||index>=service_->candidate_count())return E_INVALIDARG; selected_=index; service_->HighlightCandidate(index); return S_OK; }
STDMETHODIMP HeshunCandidateList::Finalize() { return service_?service_->FinalizeCandidate(selected_):E_FAIL; }
STDMETHODIMP HeshunCandidateList::Abort() { return service_?service_->AbortCandidate():E_FAIL; }
STDMETHODIMP HeshunCandidateList::SetIntegrationStyle(GUID) { return S_OK; }
STDMETHODIMP HeshunCandidateList::GetSelectionStyle(TfIntegratableCandidateListSelectionStyle* s) { if(!s)return E_INVALIDARG; *s=style_; return S_OK; }
STDMETHODIMP HeshunCandidateList::OnKeyDown(WPARAM, LPARAM, BOOL* eaten) { if(!eaten)return E_INVALIDARG; *eaten=FALSE; return S_OK; }
STDMETHODIMP HeshunCandidateList::ShowCandidateNumbers(BOOL* show) { if(!show)return E_INVALIDARG; *show=TRUE; return S_OK; }
STDMETHODIMP HeshunCandidateList::FinalizeExactCompositionString() { return service_?service_->FinalizeCandidate(selected_):E_FAIL; }
HRESULT HeshunCandidateList::Begin() { if(shown_)return S_OK; ITfUIElementMgr* mgr=nullptr; HRESULT hr=service_->QueryUIElementMgr(&mgr); if(SUCCEEDED(hr)){BOOL show=TRUE; hr=mgr->BeginUIElement(this,&show,&ui_id_); mgr->Release(); if(SUCCEEDED(hr))shown_=true;} return hr; }
void HeshunCandidateList::End() { if(!shown_||!service_)return; ITfUIElementMgr* mgr=nullptr; if(SUCCEEDED(service_->QueryUIElementMgr(&mgr))){mgr->EndUIElement(ui_id_);mgr->Release();} shown_=false;ui_id_=0; }
void HeshunCandidateList::Update() { if(shown_){ITfUIElementMgr* mgr=nullptr;if(SUCCEEDED(service_->QueryUIElementMgr(&mgr))){mgr->UpdateUIElement(ui_id_);mgr->Release();}} }

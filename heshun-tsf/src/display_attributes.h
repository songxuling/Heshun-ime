#pragma once

#include <windows.h>
#include <msctf.h>
#include <new>

#include "guids.h"

class HeshunDisplayAttributeInfo final : public ITfDisplayAttributeInfo {
public:
    STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
        if (!object) return E_INVALIDARG;
        *object = nullptr;
        if (riid == IID_IUnknown || riid == IID_ITfDisplayAttributeInfo) {
            *object = static_cast<ITfDisplayAttributeInfo*>(this);
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
    STDMETHODIMP GetGUID(GUID* guid) override {
        if (!guid) return E_INVALIDARG;
        *guid = GUID_DISPLAYATTRIBUTE_HESHUN_PREEDIT;
        return S_OK;
    }
    STDMETHODIMP GetDescription(BSTR* description) override {
        if (!description) return E_INVALIDARG;
        *description = SysAllocString(L"heshun preedit");
        return *description ? S_OK : E_OUTOFMEMORY;
    }
    STDMETHODIMP GetAttributeInfo(TF_DISPLAYATTRIBUTE* attribute) override {
        if (!attribute) return E_INVALIDARG;
        *attribute = attribute_;
        return S_OK;
    }
    STDMETHODIMP SetAttributeInfo(const TF_DISPLAYATTRIBUTE*) override { return E_NOTIMPL; }
    STDMETHODIMP Reset() override { return SetAttributeInfo(&attribute_); }

private:
    LONG ref_count_ = 1;
    TF_DISPLAYATTRIBUTE attribute_ = [] {
        TF_DISPLAYATTRIBUTE value{};
        value.crText.type = TF_CT_NONE;
        value.crBk.type = TF_CT_NONE;
        value.lsStyle = TF_LS_DOT;
        value.fBoldLine = FALSE;
        // Weasel leaves the line color to the host with TF_CT_NONE.  This
        // host accepts the attribute and asks the provider for its info, but
        // renders no line for that unspecified color.  Keep Weasel's dotted
        // style and input attribute while making the visible line explicit.
        value.crLine.type = TF_CT_COLORREF;
        value.crLine.cr = RGB(0, 120, 215);
        value.bAttr = TF_ATTR_INPUT;
        return value;
    }();
};

class HeshunDisplayAttributeEnum final : public IEnumTfDisplayAttributeInfo {
public:
    explicit HeshunDisplayAttributeEnum(ULONG index = 0) : index_(index) {}

    STDMETHODIMP QueryInterface(REFIID riid, void** object) override {
        if (!object) return E_INVALIDARG;
        *object = nullptr;
        if (riid == IID_IUnknown || riid == IID_IEnumTfDisplayAttributeInfo) {
            *object = static_cast<IEnumTfDisplayAttributeInfo*>(this);
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
    STDMETHODIMP Clone(IEnumTfDisplayAttributeInfo** result) override {
        if (!result) return E_INVALIDARG;
        *result = nullptr;
        auto* copy = new (std::nothrow) HeshunDisplayAttributeEnum(index_);
        if (!copy) return E_OUTOFMEMORY;
        *result = copy;
        return S_OK;
    }
    STDMETHODIMP Next(ULONG count, ITfDisplayAttributeInfo** info, ULONG* fetched) override {
        if (count == 0) return S_OK;
        if (!info) return E_INVALIDARG;
        if (fetched) *fetched = 0;
        ULONG returned = 0;
        while (returned < count && index_ == 0) {
            auto* item = new (std::nothrow) HeshunDisplayAttributeInfo();
            if (!item) return E_OUTOFMEMORY;
            info[returned++] = item;
            index_ = 1;
        }
        if (fetched) *fetched = returned;
        return returned == count ? S_OK : S_FALSE;
    }
    STDMETHODIMP Reset() override { index_ = 0; return S_OK; }
    STDMETHODIMP Skip(ULONG count) override {
        index_ = (count || index_) ? 1 : 0;
        return index_ == 1 ? S_FALSE : S_OK;
    }

private:
    LONG ref_count_ = 1;
    ULONG index_ = 0;
};

inline HRESULT CreateHeshunDisplayAttributeEnum(IEnumTfDisplayAttributeInfo** result) {
    if (!result) return E_INVALIDARG;
    *result = nullptr;
    auto* value = new (std::nothrow) HeshunDisplayAttributeEnum();
    if (!value) return E_OUTOFMEMORY;
    *result = value;
    return S_OK;
}

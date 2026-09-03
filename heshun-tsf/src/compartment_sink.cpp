#include "compartment_sink.h"

HeshunCompartmentSink::HeshunCompartmentSink(std::function<void(REFGUID)> callback)
    : callback_(std::move(callback)) {}

HeshunCompartmentSink::~HeshunCompartmentSink() { Unadvise(); }

STDMETHODIMP HeshunCompartmentSink::QueryInterface(REFIID riid, void** object) {
    if (!object) return E_INVALIDARG;
    *object = nullptr;
    if (riid == IID_IUnknown || riid == IID_ITfCompartmentEventSink) {
        *object = static_cast<ITfCompartmentEventSink*>(this);
        AddRef();
        return S_OK;
    }
    return E_NOINTERFACE;
}

STDMETHODIMP_(ULONG) HeshunCompartmentSink::AddRef() { return InterlockedIncrement(&ref_count_); }
STDMETHODIMP_(ULONG) HeshunCompartmentSink::Release() {
    const ULONG count = InterlockedDecrement(&ref_count_);
    if (!count) delete this;
    return count;
}

STDMETHODIMP HeshunCompartmentSink::OnChange(REFGUID guid) {
    if (callback_) callback_(guid);
    return S_OK;
}

HRESULT HeshunCompartmentSink::Advise(ITfCompartment* compartment) {
    if (!compartment) return E_INVALIDARG;
    ITfSource* source = nullptr;
    HRESULT hr = compartment->QueryInterface(IID_PPV_ARGS(&source));
    if (SUCCEEDED(hr)) {
        hr = source->AdviseSink(IID_ITfCompartmentEventSink, this, &cookie_);
        source->Release();
    }
    if (SUCCEEDED(hr)) {
        compartment_ = compartment;
        compartment_->AddRef();
    }
    return hr;
}

void HeshunCompartmentSink::Unadvise() {
    if (compartment_ && cookie_ != TF_INVALID_COOKIE) {
        ITfSource* source = nullptr;
        if (SUCCEEDED(compartment_->QueryInterface(IID_PPV_ARGS(&source)))) {
            source->UnadviseSink(cookie_);
            source->Release();
        }
    }
    cookie_ = TF_INVALID_COOKIE;
    if (compartment_) {
        compartment_->Release();
        compartment_ = nullptr;
    }
}

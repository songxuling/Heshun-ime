#pragma once
#include <windows.h>
#include <msctf.h>
#include <functional>

class HeshunCompartmentSink final : public ITfCompartmentEventSink {
public:
    explicit HeshunCompartmentSink(std::function<void(REFGUID)> callback);
    ~HeshunCompartmentSink();
    STDMETHODIMP QueryInterface(REFIID riid, void** object) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;
    STDMETHODIMP OnChange(REFGUID guid) override;
    HRESULT Advise(ITfCompartment* compartment);
    void Unadvise();
private:
    LONG ref_count_ = 1;
    ITfCompartment* compartment_ = nullptr;
    DWORD cookie_ = TF_INVALID_COOKIE;
    std::function<void(REFGUID)> callback_;
};

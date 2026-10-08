#pragma once
#include <windows.h>
#include <msctf.h>
#include <wrl/client.h>
inline bool compartment(IUnknown* owner, REFGUID id) {
    Microsoft::WRL::ComPtr<ITfCompartmentMgr> manager;
    Microsoft::WRL::ComPtr<ITfCompartment> value; VARIANT variant; VariantInit(&variant);
    if (FAILED(owner->QueryInterface(IID_PPV_ARGS(&manager))) || FAILED(manager->GetCompartment(id,&value)) || FAILED(value->GetValue(&variant))) return false;
    bool result = variant.vt == VT_I4 && variant.lVal != 0; VariantClear(&variant); return result;
}
inline bool shiftKey(WPARAM key) { return key == VK_SHIFT || key == VK_LSHIFT || key == VK_RSHIFT; }

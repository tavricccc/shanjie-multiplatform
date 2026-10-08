#include "service.h"
#include "identity.h"
#include <string>
HMODULE module = nullptr;
long objects = 0;
namespace {
class Factory final : public IClassFactory {
    long refs_ = 1;
public:
    Factory() { InterlockedIncrement(&objects); }
    ~Factory() { InterlockedDecrement(&objects); }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override { *out = nullptr; if (iid != IID_IUnknown && iid != IID_IClassFactory) return E_NOINTERFACE; *out = this; AddRef(); return S_OK; }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer, REFIID iid, void** out) override { if (outer) return CLASS_E_NOAGGREGATION; auto service = new Service; auto hr = service->QueryInterface(iid,out); service->Release(); return hr; }
    HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override { if (lock) InterlockedIncrement(&objects); else InterlockedDecrement(&objects); return S_OK; }
};
}
extern "C" HRESULT sj_register(intptr_t,bool);
BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID) { if (reason == DLL_PROCESS_ATTACH) { module = instance; DisableThreadLibraryCalls(instance); } return TRUE; }
STDAPI DllCanUnloadNow() { return objects == 0 ? S_OK : S_FALSE; }
STDAPI DllGetClassObject(REFCLSID id, REFIID iid, void** out) { if (id != ServiceId) return CLASS_E_CLASSNOTAVAILABLE; auto factory = new Factory; auto hr = factory->QueryInterface(iid,out); factory->Release(); return hr; }
STDAPI DllUnregisterServer() {
    return sj_register((intptr_t)module,true);
}
STDAPI DllRegisterServer() {
    HRESULT hr=sj_register((intptr_t)module,false);
    if (FAILED(hr)) DllUnregisterServer();
    return hr;
}

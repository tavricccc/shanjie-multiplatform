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
std::wstring registryPath() { wchar_t guid[40]; StringFromGUID2(ServiceId,guid,40); return std::wstring(L"Software\\Classes\\CLSID\\") + guid; }
HRESULT setValue(const std::wstring& key, const wchar_t* name, const std::wstring& value) {
    HKEY handle; auto error = RegCreateKeyExW(HKEY_LOCAL_MACHINE,key.c_str(),0,nullptr,0,KEY_WRITE,nullptr,&handle,nullptr);
    if (error != ERROR_SUCCESS) return HRESULT_FROM_WIN32(error);
    error = RegSetValueExW(handle,name,0,REG_SZ,(const BYTE*)value.c_str(),(DWORD)((value.size()+1)*sizeof(wchar_t)));
    RegCloseKey(handle); return HRESULT_FROM_WIN32(error);
}
const GUID* categories[] = { &GUID_TFCAT_TIP_KEYBOARD, &GUID_TFCAT_TIPCAP_UIELEMENTENABLED, &GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT, &GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT };
}
BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID) { if (reason == DLL_PROCESS_ATTACH) { module = instance; DisableThreadLibraryCalls(instance); } return TRUE; }
STDAPI DllCanUnloadNow() { return objects == 0 ? S_OK : S_FALSE; }
STDAPI DllGetClassObject(REFCLSID id, REFIID iid, void** out) { if (id != ServiceId) return CLASS_E_CLASSNOTAVAILABLE; auto factory = new Factory; auto hr = factory->QueryInterface(iid,out); factory->Release(); return hr; }
STDAPI DllUnregisterServer() {
    HRESULT initialized = CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);
    Microsoft::WRL::ComPtr<ITfInputProcessorProfiles> profiles;
    HRESULT hr = CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&profiles));
    if (SUCCEEDED(hr)) hr = profiles->Unregister(ServiceId);
    Microsoft::WRL::ComPtr<ITfCategoryMgr> manager;
    if (SUCCEEDED(CoCreateInstance(CLSID_TF_CategoryMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&manager)))) for (auto category : categories) manager->UnregisterCategory(ServiceId,*category,ServiceId);
    RegDeleteTreeW(HKEY_LOCAL_MACHINE,registryPath().c_str());
    if (SUCCEEDED(initialized)) CoUninitialize(); return hr;
}
STDAPI DllRegisterServer() {
    wchar_t path[32768]; auto length = GetModuleFileNameW(module,path,32768); if (!length || length == 32768) return E_FAIL;
    HRESULT hr = setValue(registryPath(),nullptr,L"善解 Windows");
    if (SUCCEEDED(hr)) hr = setValue(registryPath()+L"\\InprocServer32",nullptr,path);
    if (SUCCEEDED(hr)) hr = setValue(registryPath()+L"\\InprocServer32",L"ThreadingModel",L"Apartment");
    if (FAILED(hr)) return hr;
    HRESULT initialized = CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);
    Microsoft::WRL::ComPtr<ITfInputProcessorProfiles> profiles;
    hr = CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&profiles));
    if (SUCCEEDED(hr)) hr = profiles->Register(ServiceId);
    if (SUCCEEDED(hr)) hr = profiles->AddLanguageProfile(ServiceId,Language,StandardProfile,L"善解注音（標準）",(ULONG)wcslen(L"善解注音（標準）"),path,length,0);
    if (SUCCEEDED(hr)) hr = profiles->AddLanguageProfile(ServiceId,Language,EtenProfile,L"善解注音（倚天）",(ULONG)wcslen(L"善解注音（倚天）"),path,length,0);
    if (SUCCEEDED(hr)) hr = profiles->EnableLanguageProfile(ServiceId,Language,StandardProfile,TRUE);
    if (SUCCEEDED(hr)) hr = profiles->EnableLanguageProfile(ServiceId,Language,EtenProfile,TRUE);
    Microsoft::WRL::ComPtr<ITfCategoryMgr> manager;
    if (SUCCEEDED(hr)) hr = CoCreateInstance(CLSID_TF_CategoryMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&manager));
    if (SUCCEEDED(hr)) for (auto category : categories) { hr = manager->RegisterCategory(ServiceId,*category,ServiceId); if (FAILED(hr)) break; }
    if (SUCCEEDED(initialized)) CoUninitialize();
    if (FAILED(hr)) DllUnregisterServer();
    return hr;
}

#pragma once
// Only keyboard-sink registration is substituted: Windows requires a registered
// active TIP for AdviseKeyEventSink. Everything else uses the real TSF manager.
class ThreadProxy final : public ITfThreadMgr, public ITfKeystrokeMgr {
    long refs_ = 1;
    ComPtr<ITfThreadMgr> real_;
    ComPtr<ITfKeystrokeMgr> keys_;
    ComPtr<ITfKeyEventSink> sink_;
public:
    ThreadProxy(ITfThreadMgr* real) : real_(real) { real->QueryInterface(IID_PPV_ARGS(&keys_)); }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override {
        if (iid == IID_IUnknown || iid == IID_ITfThreadMgr) *out = static_cast<ITfThreadMgr*>(this);
        else if (iid == IID_ITfKeystrokeMgr) *out = static_cast<ITfKeystrokeMgr*>(this);
        else return real_->QueryInterface(iid,out);
        AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE Activate(TfClientId* id) override { return real_->Activate(id); }
    HRESULT STDMETHODCALLTYPE Deactivate() override { return real_->Deactivate(); }
    HRESULT STDMETHODCALLTYPE CreateDocumentMgr(ITfDocumentMgr** out) override { return real_->CreateDocumentMgr(out); }
    HRESULT STDMETHODCALLTYPE EnumDocumentMgrs(IEnumTfDocumentMgrs** out) override { return real_->EnumDocumentMgrs(out); }
    HRESULT STDMETHODCALLTYPE GetFocus(ITfDocumentMgr** out) override { return real_->GetFocus(out); }
    HRESULT STDMETHODCALLTYPE SetFocus(ITfDocumentMgr* doc) override { return real_->SetFocus(doc); }
    HRESULT STDMETHODCALLTYPE AssociateFocus(HWND hwnd,ITfDocumentMgr* doc,ITfDocumentMgr** previous) override { return real_->AssociateFocus(hwnd,doc,previous); }
    HRESULT STDMETHODCALLTYPE IsThreadFocus(BOOL* out) override { return real_->IsThreadFocus(out); }
    HRESULT STDMETHODCALLTYPE GetFunctionProvider(REFCLSID id,ITfFunctionProvider** out) override { return real_->GetFunctionProvider(id,out); }
    HRESULT STDMETHODCALLTYPE EnumFunctionProviders(IEnumTfFunctionProviders** out) override { return real_->EnumFunctionProviders(out); }
    HRESULT STDMETHODCALLTYPE GetGlobalCompartment(ITfCompartmentMgr** out) override { return real_->GetGlobalCompartment(out); }
    HRESULT STDMETHODCALLTYPE AdviseKeyEventSink(TfClientId,ITfKeyEventSink* sink,BOOL) override { sink_ = sink; return S_OK; }
    HRESULT STDMETHODCALLTYPE UnadviseKeyEventSink(TfClientId) override { sink_.Reset(); return S_OK; }
    HRESULT STDMETHODCALLTYPE GetForeground(CLSID* out) override { return keys_->GetForeground(out); }
    HRESULT STDMETHODCALLTYPE TestKeyDown(WPARAM w,LPARAM l,BOOL* out) override { return keys_->TestKeyDown(w,l,out); }
    HRESULT STDMETHODCALLTYPE TestKeyUp(WPARAM w,LPARAM l,BOOL* out) override { return keys_->TestKeyUp(w,l,out); }
    HRESULT STDMETHODCALLTYPE KeyDown(WPARAM w,LPARAM l,BOOL* out) override { return keys_->KeyDown(w,l,out); }
    HRESULT STDMETHODCALLTYPE KeyUp(WPARAM w,LPARAM l,BOOL* out) override { return keys_->KeyUp(w,l,out); }
    HRESULT STDMETHODCALLTYPE GetPreservedKey(ITfContext* c,const TF_PRESERVEDKEY* k,GUID* out) override { return keys_->GetPreservedKey(c,k,out); }
    HRESULT STDMETHODCALLTYPE IsPreservedKey(REFGUID id,const TF_PRESERVEDKEY* k,BOOL* out) override { return keys_->IsPreservedKey(id,k,out); }
    HRESULT STDMETHODCALLTYPE PreserveKey(TfClientId c,REFGUID id,const TF_PRESERVEDKEY* k,const WCHAR* desc,ULONG len) override { return keys_->PreserveKey(c,id,k,desc,len); }
    HRESULT STDMETHODCALLTYPE UnpreserveKey(REFGUID id,const TF_PRESERVEDKEY* k) override { return keys_->UnpreserveKey(id,k); }
    HRESULT STDMETHODCALLTYPE SetPreservedKeyDescription(REFGUID id,const WCHAR* desc,ULONG len) override { return keys_->SetPreservedKeyDescription(id,desc,len); }
    HRESULT STDMETHODCALLTYPE GetPreservedKeyDescription(REFGUID id,BSTR* out) override { return keys_->GetPreservedKeyDescription(id,out); }
    HRESULT STDMETHODCALLTYPE SimulatePreservedKey(ITfContext* c,REFGUID id,BOOL* out) override { return keys_->SimulatePreservedKey(c,id,out); }
};

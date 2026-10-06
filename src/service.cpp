#include "service.h"
#include "identity.h"
#include <functional>
using Microsoft::WRL::ComPtr;
namespace {
class Edit final : public ITfEditSession {
    long refs_ = 1;
    ComPtr<IUnknown> owner_;
    std::function<HRESULT(TfEditCookie)> operation_;
public:
    Edit(IUnknown* owner, std::function<HRESULT(TfEditCookie)> operation) : owner_(owner), operation_(std::move(operation)) { InterlockedIncrement(&objects); }
    ~Edit() { InterlockedDecrement(&objects); }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override { *out = nullptr; if (iid != IID_IUnknown && iid != IID_ITfEditSession) return E_NOINTERFACE; *out = this; AddRef(); return S_OK; }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE DoEditSession(TfEditCookie cookie) override { return operation_(cookie); }
};
bool compartment(IUnknown* owner, REFGUID id) {
    ComPtr<ITfCompartmentMgr> manager; ComPtr<ITfCompartment> value; VARIANT variant; VariantInit(&variant);
    if (FAILED(owner->QueryInterface(IID_PPV_ARGS(&manager))) || FAILED(manager->GetCompartment(id,&value)) || FAILED(value->GetValue(&variant))) return false;
    bool result = variant.vt == VT_I4 && variant.lVal != 0; VariantClear(&variant); return result;
}
}
Service::Service() { InterlockedIncrement(&objects); }
Service::~Service() { Deactivate(); InterlockedDecrement(&objects); }
HRESULT Service::QueryInterface(REFIID iid, void** out) {
    *out = nullptr;
    if (iid == IID_IUnknown || iid == IID_ITfTextInputProcessor || iid == IID_ITfTextInputProcessorEx) *out = static_cast<ITfTextInputProcessorEx*>(this);
    else if (iid == IID_ITfKeyEventSink) *out = static_cast<ITfKeyEventSink*>(this);
    else if (iid == IID_ITfCompositionSink) *out = static_cast<ITfCompositionSink*>(this);
    else if (iid == IID_ITfThreadMgrEventSink) *out = static_cast<ITfThreadMgrEventSink*>(this);
    if (!*out) return E_NOINTERFACE; AddRef(); return S_OK;
}
ULONG Service::AddRef() { return InterlockedIncrement(&refs_); }
ULONG Service::Release() { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
HRESULT Service::Activate(ITfThreadMgr* thread, TfClientId client) { return ActivateEx(thread,client,0); }
HRESULT Service::ActivateEx(ITfThreadMgr* thread, TfClientId client, DWORD) {
    thread_ = thread; client_ = client;
    wchar_t path[32768]; auto length = GetModuleFileNameW(module,path,32768);
    if (!length || length == 32768) { Deactivate(); return E_FAIL; }
    unsigned layout = 0;
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    TF_INPUTPROCESSORPROFILE active{};
    if (SUCCEEDED(CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&profiles))) && SUCCEEDED(profiles->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD,&active)) && active.guidProfile == EtenProfile) layout = 1;
    if (!engine_.open(std::filesystem::path(path).parent_path() / L"data",layout)) { Deactivate(); return E_FAIL; }
    ComPtr<ITfCompartmentMgr> compartments; ComPtr<ITfCompartment> open;
    if (SUCCEEDED(thread->QueryInterface(IID_PPV_ARGS(&compartments))) && SUCCEEDED(compartments->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&open))) {
        VARIANT value; VariantInit(&value); value.vt = VT_I4; value.lVal = 1; open->SetValue(client,&value);
    }
    candidates_.Attach(new Candidates);
    candidates_->initialize(thread,[this](unsigned index) {
        if (!context_) return;
        ComPtr<ITfContext> context = context_;
        request(context.Get(),[this,context,index](TfEditCookie cookie) {
            shanjie::Snapshot out;
            if (!engine_.pick(index,out)) return E_FAIL;
            return apply(context.Get(),cookie,out);
        }, TF_ES_READWRITE | TF_ES_ASYNCDONTCARE);
    });
    ComPtr<ITfKeystrokeMgr> keys; ComPtr<ITfSource> source;
    HRESULT hr = thread->QueryInterface(IID_PPV_ARGS(&keys));
    if (SUCCEEDED(hr)) hr = keys->AdviseKeyEventSink(client,this,TRUE);
    if (SUCCEEDED(hr)) hr = thread->QueryInterface(IID_PPV_ARGS(&source));
    if (SUCCEEDED(hr)) hr = source->AdviseSink(IID_ITfThreadMgrEventSink,static_cast<ITfThreadMgrEventSink*>(this),&threadSink_);
    if (FAILED(hr)) Deactivate();
    return hr;
}
HRESULT Service::Deactivate() {
    cancel();
    if (thread_) {
        ComPtr<ITfKeystrokeMgr> keys; if (SUCCEEDED(thread_.As(&keys))) keys->UnadviseKeyEventSink(client_);
        ComPtr<ITfSource> source; if (threadSink_ != TF_INVALID_COOKIE && SUCCEEDED(thread_.As(&source))) source->UnadviseSink(threadSink_);
    }
    threadSink_ = TF_INVALID_COOKIE; candidates_.Reset(); thread_.Reset(); client_ = TF_CLIENTID_NULL; return S_OK;
}
HRESULT Service::request(ITfContext* context, std::function<HRESULT(TfEditCookie)> operation, DWORD flags) {
    ComPtr<ITfEditSession> edit; edit.Attach(new Edit(static_cast<ITfTextInputProcessorEx*>(this),std::move(operation)));
    HRESULT result = E_FAIL;
    HRESULT hr = context->RequestEditSession(client_,edit.Get(),flags,&result);
    return FAILED(hr) ? hr : result;
}
void Service::cancel() {
    if (composition_ && context_ && client_ != TF_CLIENTID_NULL) {
        ComPtr<ITfComposition> composition = composition_; ComPtr<ITfContext> context = context_;
        request(context.Get(),[composition](TfEditCookie cookie) { return composition->EndComposition(cookie); },TF_ES_READWRITE | TF_ES_ASYNCDONTCARE);
    }
    composition_.Reset(); context_.Reset(); if (candidates_) candidates_->close();
    shanjie::Snapshot discarded; engine_.reset(true,discarded); state_ = {};
}
bool Service::enabled(ITfContext* context) {
    if (compartment(context,GUID_COMPARTMENT_KEYBOARD_DISABLED) || compartment(context,GUID_COMPARTMENT_EMPTYCONTEXT)) return false;
    return !compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) ? false : true;
}
bool Service::wants(ITfContext* context, WPARAM vk, LPARAM flags) {
    if (!enabled(context)) return false;
    ShanjieKey key{}; if (!shanjie::translate(vk,flags,key)) return false;
    if (!state_.preedit.empty()) return true;
    return key.kind == 1 && !(key.modifiers & 30);
}
HRESULT Service::apply(ITfContext* context, TfEditCookie cookie, const shanjie::Snapshot& out) {
    ComPtr<ITfRange> range;
    if (composition_) {
        HRESULT hr = composition_->GetRange(&range); if (FAILED(hr)) return hr;
    } else {
        ComPtr<ITfInsertAtSelection> insert;
        HRESULT hr = context->QueryInterface(IID_PPV_ARGS(&insert)); if (FAILED(hr)) return hr;
        hr = insert->InsertTextAtSelection(cookie,TF_IAS_QUERYONLY,nullptr,0,&range); if (FAILED(hr)) return hr;
    }
    if (!out.handled && out.commit.empty() && out.preedit.empty()) return S_OK;
    auto text = out.commit + out.preedit;
    HRESULT hr = range->SetText(cookie,0,text.data(),(LONG)text.size()); if (FAILED(hr)) return hr;
    if (!out.commit.empty()) {
        LONG moved = 0; hr = range->ShiftStart(cookie,(LONG)out.commit.size(),&moved,nullptr); if (FAILED(hr)) return hr;
        if (composition_) { ending_ = true; hr = composition_->EndComposition(cookie); ending_ = false; composition_.Reset(); if (FAILED(hr)) return hr; }
    }
    if (!out.preedit.empty() && !composition_) {
        ComPtr<ITfContextComposition> composing; hr = context->QueryInterface(IID_PPV_ARGS(&composing));
        if (SUCCEEDED(hr)) hr = composing->StartComposition(cookie,range.Get(),this,&composition_);
        if (FAILED(hr)) return hr;
    }
    if (out.preedit.empty() && composition_) { ending_ = true; hr = composition_->EndComposition(cookie); ending_ = false; composition_.Reset(); if (FAILED(hr)) return hr; }
    ComPtr<ITfRange> caret; hr = range->Clone(&caret); if (FAILED(hr)) return hr;
    caret->Collapse(cookie,TF_ANCHOR_START); LONG moved = 0;
    caret->ShiftEnd(cookie,(LONG)out.cursor,&moved,nullptr); caret->Collapse(cookie,TF_ANCHOR_END);
    TF_SELECTION selection{caret.Get(),{TF_AE_NONE,FALSE}};
    hr = context->SetSelection(cookie,1,&selection); if (FAILED(hr)) return hr;
    state_ = out; context_ = context;
    candidates_->update(context,cookie,caret.Get(),out);
    return S_OK;
}
HRESULT Service::OnKeyDown(ITfContext* context, WPARAM vk, LPARAM flags, BOOL* eaten) {
    *eaten = FALSE; if (!wants(context,vk,flags)) return S_OK;
    ShanjieKey key{}; shanjie::translate(vk,flags,key);
    bool handled = false;
    HRESULT hr = request(context,[this,context,key,&handled](TfEditCookie cookie) {
        if (state_.preedit.empty()) {
            TF_SELECTION selection{}; ULONG fetched = 0;
            if (SUCCEEDED(context->GetSelection(cookie,TF_DEFAULT_SELECTION,1,&selection,&fetched)) && fetched) {
                ComPtr<ITfRange> left; selection.range->Clone(&left); selection.range->Release();
                left->Collapse(cookie,TF_ANCHOR_START); LONG moved = 0; left->ShiftStart(cookie,-8,&moved,nullptr);
                WCHAR text[8]; ULONG count = 0;
                if (SUCCEEDED(left->GetText(cookie,0,text,8,&count))) engine_.context(std::wstring(text,count));
            }
        }
        shanjie::Snapshot out;
        if (!engine_.key(key,out)) return E_FAIL;
        HRESULT result = apply(context,cookie,out); handled = SUCCEEDED(result) && out.handled; return result;
    },TF_ES_SYNC | TF_ES_READWRITE);
    if (FAILED(hr)) { cancel(); return S_OK; }
    *eaten = handled; return S_OK;
}
HRESULT Service::OnTestKeyDown(ITfContext* c, WPARAM k, LPARAM f, BOOL* out) { *out = wants(c,k,f); return S_OK; }
HRESULT Service::OnTestKeyUp(ITfContext*, WPARAM, LPARAM, BOOL* out) { *out = FALSE; return S_OK; }
HRESULT Service::OnKeyUp(ITfContext*, WPARAM, LPARAM, BOOL* out) { *out = FALSE; return S_OK; }
HRESULT Service::OnPreservedKey(ITfContext*, REFGUID, BOOL* out) { *out = FALSE; return S_OK; }
HRESULT Service::OnCompositionTerminated(TfEditCookie, ITfComposition* ended) {
    if (!ending_ && composition_.Get() == ended) { composition_.Reset(); context_.Reset(); if (candidates_) candidates_->close(); shanjie::Snapshot out; engine_.reset(true,out); state_ = {}; }
    return S_OK;
}
HRESULT Service::OnSetFocus(BOOL focused) { if (!focused) cancel(); return S_OK; }
HRESULT Service::OnInitDocumentMgr(ITfDocumentMgr*) { return S_OK; }
HRESULT Service::OnUninitDocumentMgr(ITfDocumentMgr*) { return S_OK; }
HRESULT Service::OnSetFocus(ITfDocumentMgr* current, ITfDocumentMgr* previous) { if (current != previous) cancel(); return S_OK; }
HRESULT Service::OnPushContext(ITfContext*) { return S_OK; }
HRESULT Service::OnPopContext(ITfContext* context) { if (context_.Get() == context) cancel(); return S_OK; }

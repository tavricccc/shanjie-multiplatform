#include "candidates.h"
#include "identity.h"
#include <algorithm>
using Microsoft::WRL::ComPtr;
Candidates::Candidates() { InterlockedIncrement(&objects); }
Candidates::~Candidates() { close(); if (window_) DestroyWindow(window_); InterlockedDecrement(&objects); }
HRESULT Candidates::QueryInterface(REFIID iid, void** out) {
    *out = nullptr;
    if (iid == IID_IUnknown || iid == IID_ITfUIElement || iid == IID_ITfCandidateListUIElement || iid == IID_ITfCandidateListUIElementBehavior) *out = static_cast<ITfCandidateListUIElementBehavior*>(this);
    if (!*out) return E_NOINTERFACE;
    AddRef(); return S_OK;
}
ULONG Candidates::AddRef() { return InterlockedIncrement(&refs_); }
ULONG Candidates::Release() { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
void Candidates::initialize(ITfThreadMgr* thread, const Preferences& preferences, std::function<void(unsigned)> pick) {
    thread->QueryInterface(IID_PPV_ARGS(&manager_)); preferences_ = preferences; pick_ = std::move(pick);
    if (prepareHost()) host_.hide();
}
void Candidates::close() {
    ++revision_;
    host_.hide();
    if (manager_ && id_ != TF_INVALID_UIELEMENTID) { manager_->EndUIElement(id_); id_ = TF_INVALID_UIELEMENTID; }
    document_.Reset(); state_ = {};
}
void Candidates::update(ITfContext* context, TfEditCookie cookie, ITfRange* range, const shanjie::Snapshot& state) {
    if (state.candidates.empty()) { close(); return; }
    ++revision_;
    state_ = state; context->GetDocumentMgr(&document_);
    if (manager_ && id_ == TF_INVALID_UIELEMENTID) { BOOL show = TRUE; manager_->BeginUIElement(this,&show,&id_); native_ = show != FALSE; }
    if (manager_ && id_ != TF_INVALID_UIELEMENTID) manager_->UpdateUIElement(id_);
    if (!native_) { host_.hide(); return; }
    ComPtr<ITfContextView> view; BOOL clipped = FALSE; HWND owner = nullptr;
    if (FAILED(context->GetActiveView(&view)) || FAILED(view->GetTextExt(cookie,range,&caret_,&clipped)) || clipped) { Show(FALSE); return; }
    view->GetWnd(&owner); dpi_ = owner ? GetDpiForWindow(owner) : 96;
    // WinUI runs in a per-monitor-aware process; legacy host apps may report virtualized coordinates.
    if (owner && GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(owner)) != DPI_AWARENESS_PER_MONITOR_AWARE) {
        POINT start{caret_.left,caret_.top}, end{caret_.right,caret_.bottom};
        if (!LogicalToPhysicalPointForPerMonitorDPI(owner,&start) || !LogicalToPhysicalPointForPerMonitorDPI(owner,&end)) { Show(FALSE); return; }
        caret_ = {start.x,start.y,end.x,end.y};
    }
    if (prepareHost()) host_.publish(state_,preferences_,caret_,dpi_,true);
}
bool Candidates::prepareHost() {
    if (GetEnvironmentVariableW(L"SHANJIE_HEADLESS",nullptr,0)) return false;
    if (!window_) {
        WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = module; wc.lpszClassName = L"ShanjieCandidateChannel";
        RegisterClassW(&wc);
        window_ = CreateWindowExW(0,wc.lpszClassName,L"",0,0,0,0,0,HWND_MESSAGE,nullptr,module,this);
    }
    return window_ && host_.start(window_);
}
LRESULT Candidates::procedure(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto self = reinterpret_cast<Candidates*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (message == WM_NCCREATE) { self = (Candidates*)((CREATESTRUCTW*)lp)->lpCreateParams; SetWindowLongPtrW(window,GWLP_USERDATA,(LONG_PTR)self); }
    if (self && message == WM_APP + 42) {
        // Reject a click from a frame that was replaced or closed before the edit session.
        if ((LONG)lp == self->host_.sequence() && (wp < self->state_.candidates.size() || (wp == self->state_.candidates.size() && !self->state_.reading.empty())) && self->host_.shown()) self->pick_((unsigned)wp);
        return 0;
    }
    if (self && message == WM_APP + 43) { self->host_.ready((HWND)wp); return 0; }
    return DefWindowProcW(window,message,wp,lp);
}
HRESULT Candidates::GetDescription(BSTR* out) { *out = SysAllocString(L"善解候選"); return *out ? S_OK : E_OUTOFMEMORY; }
HRESULT Candidates::GetGUID(GUID* out) { *out = ServiceId; return S_OK; }
HRESULT Candidates::Show(BOOL value) { if (value && native_) host_.publish(state_,preferences_,caret_,dpi_,true); else host_.hide(); return S_OK; }
HRESULT Candidates::IsShown(BOOL* out) { *out = host_.shown(); return S_OK; }
HRESULT Candidates::GetUpdatedFlags(DWORD* out) { *out = TF_CLUIE_COUNT | TF_CLUIE_SELECTION | TF_CLUIE_STRING | TF_CLUIE_PAGEINDEX | TF_CLUIE_CURRENTPAGE; return S_OK; }
HRESULT Candidates::GetDocumentMgr(ITfDocumentMgr** out) { return document_.CopyTo(out); }
HRESULT Candidates::GetCount(UINT* out) { *out = (UINT)state_.candidates.size(); return S_OK; }
HRESULT Candidates::GetSelection(UINT* out) { *out = state_.selected < 0 ? 0 : (UINT)state_.selected; return S_OK; }
HRESULT Candidates::GetString(UINT index, BSTR* out) { if (index >= state_.candidates.size()) return E_INVALIDARG; *out = SysAllocString(state_.candidates[index].c_str()); return *out ? S_OK : E_OUTOFMEMORY; }
HRESULT Candidates::GetPageIndex(UINT* out, UINT size, UINT* count) { *count = ((UINT)state_.candidates.size() + 8) / 9; for (UINT i = 0; i < std::min(size,*count); ++i) out[i] = i * 9; return size < *count ? S_FALSE : S_OK; }
HRESULT Candidates::SetPageIndex(UINT*, UINT) { return E_NOTIMPL; }
HRESULT Candidates::GetCurrentPage(UINT* out) { *out = state_.selected < 0 ? 0 : (UINT)state_.selected / 9; return S_OK; }
HRESULT Candidates::SetSelection(UINT index) { if (index >= state_.candidates.size()) return E_INVALIDARG; ++revision_; state_.selected = (int)index; host_.publish(state_,preferences_,caret_,dpi_,native_); return S_OK; }
HRESULT Candidates::Finalize() { if (state_.selected >= 0) pick_((unsigned)state_.selected); return S_OK; }
HRESULT Candidates::Abort() { close(); return S_OK; }

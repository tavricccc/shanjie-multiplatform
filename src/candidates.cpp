#include "candidates.h"
#include "identity.h"
#include <algorithm>
using Microsoft::WRL::ComPtr;
Candidates::Candidates() { InterlockedIncrement(&objects); }
Candidates::~Candidates() { close(); if (window_) DestroyWindow(window_); if (font_) DeleteObject(font_); InterlockedDecrement(&objects); }
HRESULT Candidates::QueryInterface(REFIID iid, void** out) {
    *out = nullptr;
    if (iid == IID_IUnknown || iid == IID_ITfUIElement || iid == IID_ITfCandidateListUIElement || iid == IID_ITfCandidateListUIElementBehavior) *out = static_cast<ITfCandidateListUIElementBehavior*>(this);
    if (!*out) return E_NOINTERFACE;
    AddRef(); return S_OK;
}
ULONG Candidates::AddRef() { return InterlockedIncrement(&refs_); }
ULONG Candidates::Release() { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
void Candidates::initialize(ITfThreadMgr* thread, bool vertical, std::function<void(unsigned)> pick) { thread->QueryInterface(IID_PPV_ARGS(&manager_)); vertical_ = vertical; pick_ = std::move(pick); }
void Candidates::close() {
    if (window_) ShowWindow(window_, SW_HIDE);
    if (manager_ && id_ != TF_INVALID_UIELEMENTID) { manager_->EndUIElement(id_); id_ = TF_INVALID_UIELEMENTID; }
    document_.Reset(); state_ = {};
}
void Candidates::update(ITfContext* context, TfEditCookie cookie, ITfRange* range, const shanjie::Snapshot& state) {
    if (state.candidates.empty()) { close(); return; }
    state_ = state; context->GetDocumentMgr(&document_);
    if (manager_ && id_ == TF_INVALID_UIELEMENTID) { BOOL show = TRUE; manager_->BeginUIElement(this, &show, &id_); native_ = show != FALSE; }
    if (manager_ && id_ != TF_INVALID_UIELEMENTID) manager_->UpdateUIElement(id_);
    if (!native_) return;
    ComPtr<ITfContextView> view; RECT caret{}; BOOL clipped = FALSE; HWND owner = nullptr;
    if (FAILED(context->GetActiveView(&view)) || FAILED(view->GetTextExt(cookie, range, &caret, &clipped))) { Show(FALSE); return; }
    view->GetWnd(&owner);
    if (owner) owner = GetAncestor(owner,GA_ROOT);
    int dpi = owner ? (int)GetDpiForWindow(owner) : 96;
    row_ = MulDiv(36, dpi, 96); pad_ = MulDiv(8, dpi, 96);
    if (font_) DeleteObject(font_);
    font_ = CreateFontW(-MulDiv(16, dpi, 96),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,CLEARTYPE_QUALITY,DEFAULT_PITCH,L"Microsoft JhengHei UI");
    if (!window_) {
        WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = module; wc.lpszClassName = L"ShanjieCandidates"; wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
        RegisterClassW(&wc);
        window_ = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, wc.lpszClassName, L"善解候選", WS_POPUP | WS_BORDER,0,0,0,0,owner,nullptr,module,this);
    }
    MONITORINFO info{sizeof(info)}; GetMonitorInfoW(MonitorFromRect(&caret, MONITOR_DEFAULTTONEAREST), &info);
    columns_ = !vertical_ || state.columns ? std::min<int>(9, (int)state.candidates.size()) : 1;
    HDC dc = GetDC(window_); auto old = SelectObject(dc, font_); cell_ = MulDiv(72, dpi, 96);
    for (auto& text : state.candidates) { SIZE size{}; GetTextExtentPoint32W(dc,text.data(),(int)text.size(),&size); cell_ = std::max<int>(cell_, size.cx + pad_ * 4); }
    SelectObject(dc, old); ReleaseDC(window_, dc);
    cell_ = std::min<int>(cell_, (info.rcWork.right - info.rcWork.left - pad_ * 2 - 2) / columns_);
    int width = cell_ * columns_ + pad_ * 2 + 2;
    int height = ((int)state.candidates.size() + columns_ - 1) / columns_ * row_ + pad_ * 2 + 2;
    int x = std::clamp<int>(caret.left, info.rcWork.left, info.rcWork.right - width);
    int y = caret.bottom + 4;
    if (y + height > info.rcWork.bottom) y = caret.top - height - 4;
    y = std::max<int>(y, info.rcWork.top);
    SetWindowPos(window_, HWND_TOPMOST, x,y,width,height,SWP_NOACTIVATE | SWP_SHOWWINDOW);
    InvalidateRect(window_,nullptr,FALSE);
}
void Candidates::paint() {
    PAINTSTRUCT ps{}; HDC dc = BeginPaint(window_, &ps); RECT client{}; GetClientRect(window_, &client);
    FillRect(dc, &client, GetSysColorBrush(COLOR_WINDOW)); SetBkMode(dc, TRANSPARENT); auto old = SelectObject(dc,font_);
    for (unsigned i = 0; i < state_.candidates.size(); ++i) {
        RECT box{pad_ + (int)(i % columns_) * cell_,pad_ + (int)(i / columns_) * row_,0,0}; box.right = box.left + cell_; box.bottom = box.top + row_;
        bool selected = (int)i == state_.selected;
        if (selected) FillRect(dc,&box,GetSysColorBrush(COLOR_HIGHLIGHT));
        SetTextColor(dc,GetSysColor(selected ? COLOR_HIGHLIGHTTEXT : COLOR_WINDOWTEXT));
        box.left += pad_; box.right -= pad_;
        auto label = std::to_wstring(i % 9 + 1) + L" " + state_.candidates[i];
        DrawTextW(dc,label.data(),(int)label.size(),&box,DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX);
    }
    SelectObject(dc,old); EndPaint(window_,&ps);
}
LRESULT Candidates::procedure(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto self = reinterpret_cast<Candidates*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (message == WM_NCCREATE) { self = (Candidates*)((CREATESTRUCTW*)lp)->lpCreateParams; SetWindowLongPtrW(window,GWLP_USERDATA,(LONG_PTR)self); }
    if (self && message == WM_PAINT) { self->paint(); return 0; }
    if (message == WM_MOUSEACTIVATE) return MA_NOACTIVATE;
    if (self && message == WM_LBUTTONUP) {
        int x = (short)LOWORD(lp) - self->pad_, y = (short)HIWORD(lp) - self->pad_;
        if (x >= 0 && y >= 0 && x < self->columns_ * self->cell_) {
            unsigned index = y / self->row_ * self->columns_ + x / self->cell_;
            if (index < self->state_.candidates.size()) self->pick_(index);
        }
        return 0;
    }
    return DefWindowProcW(window,message,wp,lp);
}
HRESULT Candidates::GetDescription(BSTR* out) { *out = SysAllocString(L"善解候選"); return *out ? S_OK : E_OUTOFMEMORY; }
HRESULT Candidates::GetGUID(GUID* out) { *out = ServiceId; return S_OK; }
HRESULT Candidates::Show(BOOL value) { if (window_) ShowWindow(window_,value && native_ ? SW_SHOWNOACTIVATE : SW_HIDE); return S_OK; }
HRESULT Candidates::IsShown(BOOL* out) { *out = window_ && IsWindowVisible(window_); return S_OK; }
HRESULT Candidates::GetUpdatedFlags(DWORD* out) { *out = TF_CLUIE_COUNT | TF_CLUIE_SELECTION | TF_CLUIE_STRING | TF_CLUIE_PAGEINDEX | TF_CLUIE_CURRENTPAGE; return S_OK; }
HRESULT Candidates::GetDocumentMgr(ITfDocumentMgr** out) { return document_.CopyTo(out); }
HRESULT Candidates::GetCount(UINT* out) { *out = (UINT)state_.candidates.size(); return S_OK; }
HRESULT Candidates::GetSelection(UINT* out) { *out = state_.selected < 0 ? 0 : (UINT)state_.selected; return S_OK; }
HRESULT Candidates::GetString(UINT index, BSTR* out) { if (index >= state_.candidates.size()) return E_INVALIDARG; *out = SysAllocString(state_.candidates[index].c_str()); return *out ? S_OK : E_OUTOFMEMORY; }
HRESULT Candidates::GetPageIndex(UINT* out, UINT size, UINT* count) { *count = ((UINT)state_.candidates.size() + 8) / 9; for (UINT i = 0; i < std::min(size,*count); ++i) out[i] = i * 9; return size < *count ? S_FALSE : S_OK; }
HRESULT Candidates::SetPageIndex(UINT*, UINT) { return E_NOTIMPL; }
HRESULT Candidates::GetCurrentPage(UINT* out) { *out = state_.selected < 0 ? 0 : (UINT)state_.selected / 9; return S_OK; }
HRESULT Candidates::SetSelection(UINT index) { if (index >= state_.candidates.size()) return E_INVALIDARG; state_.selected = (int)index; if (window_) InvalidateRect(window_,nullptr,FALSE); return S_OK; }
HRESULT Candidates::Finalize() { if (state_.selected >= 0) pick_((unsigned)state_.selected); return S_OK; }
HRESULT Candidates::Abort() { close(); return S_OK; }

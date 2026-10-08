#include "candidate_host.h"
#include "candidate_palette.h"
#include "identity.h"
#include <windowsx.h>
#include <dwmapi.h>
#include <algorithm>
#include <cmath>
using Microsoft::WRL::ComPtr;
CandidateHost::~CandidateHost() { if (window_) DestroyWindow(window_); }
bool CandidateHost::start(HWND callback) {
    if (window_) return true;
    callback_ = callback;
    if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,factory_.GetAddressOf())) || FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED,__uuidof(IDWriteFactory),(IUnknown**)textFactory_.GetAddressOf()))) return false;
    auto properties = D2D1::RenderTargetProperties(D2D1_RENDER_TARGET_TYPE_SOFTWARE,D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM,D2D1_ALPHA_MODE_IGNORE));
    if (FAILED(factory_->CreateDCRenderTarget(&properties,&target_)) || FAILED(target_->CreateSolidColorBrush(D2D1::ColorF(0),&brush_))) return false;
    WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = module; wc.lpszClassName = L"ShanjieNativeCandidates"; RegisterClassW(&wc);
    auto awareness = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,wc.lpszClassName,L"善解候選",WS_POPUP,0,0,1,1,nullptr,nullptr,module,this);
    SetThreadDpiAwarenessContext(awareness);
    if (!window_) return false;
    int disabled = 1; DwmSetWindowAttribute(window_,DWMWA_TRANSITIONS_FORCEDISABLED,&disabled,sizeof(disabled)); return true;
}
bool CandidateHost::measure(float workWidth) {
    if (!words_.empty()) {
        fit(workWidth); return true;
    }
    ComPtr<IDWriteTextFormat> text, label;
    if (FAILED(textFactory_->CreateTextFormat(L"Microsoft JhengHei UI",nullptr,DWRITE_FONT_WEIGHT_NORMAL,DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,(float)preferences_.fontSize,L"zh-TW",&text)) || FAILED(textFactory_->CreateTextFormat(L"Segoe UI",nullptr,DWRITE_FONT_WEIGHT_NORMAL,DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,13.333f,L"zh-TW",&label))) return false;
    text->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP); label->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
    if (FAILED(textFactory_->CreateEllipsisTrimmingSign(text.Get(),ellipsis_.ReleaseAndGetAddressOf()))) return false;
    words_.clear(); labels_.clear(); float width = 0, height = 0;
    for (size_t i=0;i<state_.candidates.size();++i) {
        ComPtr<IDWriteTextLayout> word, number;
        auto& value = state_.candidates[i]; auto tag = state_.selected < 0 ? std::wstring{} : (state_.associated ? L"⇧" : L"") + std::to_wstring(i%9+1);
        if (FAILED(textFactory_->CreateTextLayout(value.data(),(UINT32)value.size(),text.Get(),10000,1000,&word)) || FAILED(textFactory_->CreateTextLayout(tag.data(),(UINT32)tag.size(),label.Get(),100,1000,&number))) return false;
        DWRITE_TEXT_METRICS metrics{}; word->GetMetrics(&metrics); width = std::max(width,metrics.widthIncludingTrailingWhitespace); height = std::max(height,metrics.height);
        words_.push_back(word); labels_.push_back(number);
    }
    textWidth_ = width; textHeight_ = height;
    fit(workWidth); return true;
}
void CandidateHost::fit(float workWidth) {
    float inset = state_.selected < 0 ? 8.0f : state_.associated ? 44.0f : 32.0f;
    cell_ = std::min(std::max(columns_ == 1 ? 96.0f : 52.0f,textWidth_+inset),(workWidth-20)/columns_);
    cell_ = std::ceil(cell_*scale_)/scale_; row_ = std::max(32.0f,textHeight_+8);
    DWRITE_TRIMMING trimming{DWRITE_TRIMMING_GRANULARITY_CHARACTER,0,0};
    for (auto& word : words_) { word->SetMaxWidth(std::max(1.0f,cell_-inset+4)); word->SetTrimming(&trimming,ellipsis_.Get()); }
}
void CandidateHost::publish(const shanjie::Snapshot& state,const Preferences& preferences,const RECT& caret,UINT,bool visible) {
    if (!window_) return;
    ++sequence_; if (!visible || state.candidates.empty()) { hide(); return; }
    if (state_.candidates != state.candidates || state_.associated != state.associated || (state_.selected < 0) != (state.selected < 0) || preferences_.fontSize != preferences.fontSize) { words_.clear(); labels_.clear(); }
    if (!paletteReady_ || preferences_.theme != preferences.theme) { palette_ = candidatePalette(preferences.theme); paletteReady_ = true; }
    state_ = state; preferences_ = preferences;
    columns_ = !state.prediction && preferences.layout == CandidateLayout::Vertical && !state.columns ? 1 : (unsigned)std::min<size_t>(9,state.candidates.size());
    MONITORINFO monitor{sizeof(monitor)}; GetMonitorInfoW(MonitorFromRect(&caret,MONITOR_DEFAULTTONEAREST),&monitor);
    if (GetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",nullptr,0)) {
        wchar_t width[16]{};
        if (GetEnvironmentVariableW(L"SHANJIE_VERIFY_WORK_WIDTH",width,16)) monitor.rcWork.right = monitor.rcWork.left+_wtoi(width);
    }
    auto awareness = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    SetWindowPos(window_,nullptr,caret.left,caret.bottom,0,0,SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER); scale_ = GetDpiForWindow(window_)/96.0f;
    if (!measure((monitor.rcWork.right-monitor.rcWork.left)/scale_)) { hide(); SetThreadDpiAwarenessContext(awareness); return; }
    int width = (int)std::ceil((cell_*columns_+20)*scale_);
    int height = std::min((int)std::ceil((row_*((words_.size()+columns_-1)/columns_)+14)*scale_),(int)(monitor.rcWork.bottom-monitor.rcWork.top));
    int x = std::clamp((int)caret.left,(int)monitor.rcWork.left,(int)monitor.rcWork.right-width);
    int y = caret.bottom+(int)(4*scale_); if (y+height > monitor.rcWork.bottom) y = caret.top-height-(int)(4*scale_); y = std::max(y,(int)monitor.rcWork.top);
    if (GetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",nullptr,0)) x = y = -12000;
    SetWindowRgn(window_,CreateRoundRectRgn(0,0,width+1,height+1,(int)(12*scale_),(int)(12*scale_)),FALSE);
    SetWindowPos(window_,HWND_TOPMOST,x,y,width,height,SWP_NOACTIVATE | SWP_SHOWWINDOW); InvalidateRect(window_,nullptr,FALSE); UpdateWindow(window_);
    SetThreadDpiAwarenessContext(awareness);
}
void CandidateHost::hide() { ++sequence_; if (window_) ShowWindow(window_,SW_HIDE); }
HRESULT CandidateHost::paint(HDC dc) {
    RECT bounds{}; GetClientRect(window_,&bounds); auto hr = target_->BindDC(dc,&bounds); if (FAILED(hr)) return hr;
    auto& palette = palette_; target_->SetDpi(96*scale_,96*scale_); target_->BeginDraw(); target_->Clear(palette.background);
    float width = bounds.right/scale_, height = bounds.bottom/scale_;
    brush_->SetColor(palette.border); target_->DrawRoundedRectangle(D2D1::RoundedRect(D2D1::RectF(.5f,.5f,width-.5f,height-.5f),6,6),brush_.Get(),1);
    for (size_t i=0;i<words_.size();++i) {
        float x = 8+(i%columns_)*cell_, y = 6+(i/columns_)*row_; bool selected = (int)i == state_.selected;
        if (selected) { brush_->SetColor(palette.accent); target_->FillRoundedRectangle(D2D1::RoundedRect(D2D1::RectF(x,y,x+cell_,y+row_),4,4),brush_.Get()); }
        brush_->SetColor(selected ? palette.selectedText : palette.text);
        target_->PushAxisAlignedClip(D2D1::RectF(x,y,x+cell_,y+row_),D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        DWRITE_TEXT_METRICS word{},number{}; words_[i]->GetMetrics(&word); labels_[i]->GetMetrics(&number);
        target_->DrawTextLayout(D2D1::Point2F(x+4,y+(row_-number.height)/2),labels_[i].Get(),brush_.Get(),D2D1_DRAW_TEXT_OPTIONS_CLIP);
        target_->DrawTextLayout(D2D1::Point2F(x+(state_.selected < 0 ? 4 : state_.associated ? 36 : 24),y+(row_-word.height)/2),words_[i].Get(),brush_.Get(),(D2D1_DRAW_TEXT_OPTIONS)(D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT));
        target_->PopAxisAlignedClip();
    }
    return target_->EndDraw();
}
LRESULT CALLBACK CandidateHost::procedure(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    auto self = (CandidateHost*)GetWindowLongPtrW(window,GWLP_USERDATA);
    if (message == WM_NCCREATE) { self = (CandidateHost*)((CREATESTRUCTW*)lp)->lpCreateParams; SetWindowLongPtrW(window,GWLP_USERDATA,(LONG_PTR)self); }
    if (message == WM_MOUSEACTIVATE) return MA_NOACTIVATE;
    if (message == WM_ERASEBKGND) return 1;
    if (self && (message == WM_PAINT || message == WM_PRINTCLIENT)) {
        PAINTSTRUCT ps{}; auto dc = message == WM_PAINT ? BeginPaint(window,&ps) : (HDC)wp;
        self->paint(dc); if (message == WM_PAINT) EndPaint(window,&ps); return 0;
    }
    if (self && message == WM_LBUTTONUP) {
        float x = GET_X_LPARAM(lp)/self->scale_-8, y = GET_Y_LPARAM(lp)/self->scale_-6;
        if (x >= 0 && y >= 0 && x < self->cell_*self->columns_) {
            auto index = (unsigned)(y/self->row_)*self->columns_+(unsigned)(x/self->cell_);
            if (index < self->words_.size()) PostMessageW(self->callback_,WM_APP+42,index,self->sequence_);
        }
        return 0;
    }
    if (self && (message == WM_SETTINGCHANGE || message == WM_THEMECHANGED || message == WM_DWMCOLORIZATIONCOLORCHANGED)) { self->palette_ = candidatePalette(self->preferences_.theme); self->paletteReady_ = true; InvalidateRect(window,nullptr,FALSE); return 0; }
    return DefWindowProcW(window,message,wp,lp);
}

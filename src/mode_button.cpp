#include "mode_button.h"
#include "identity.h"
#include <ctffunc.h>
ModeButton::ModeButton(std::function<bool()> chinese,std::function<HRESULT()> toggle,std::function<void()> settings)
    : chinese_(std::move(chinese)),toggle_(std::move(toggle)),settings_(std::move(settings)) { InterlockedIncrement(&objects); }
ModeButton::~ModeButton() { InterlockedDecrement(&objects); }
HRESULT ModeButton::QueryInterface(REFIID id,void** out) {
    *out = nullptr;
    if (id == IID_IUnknown || id == IID_ITfLangBarItem || id == IID_ITfLangBarItemButton) *out = static_cast<ITfLangBarItemButton*>(this);
    else if (id == IID_ITfSource) *out = static_cast<ITfSource*>(this);
    if (!*out) return E_NOINTERFACE; AddRef(); return S_OK;
}
ULONG ModeButton::AddRef() { return InterlockedIncrement(&refs_); }
ULONG ModeButton::Release() { auto count = InterlockedDecrement(&refs_); if (!count) delete this; return count; }
HRESULT ModeButton::GetInfo(TF_LANGBARITEMINFO* out) {
    *out = {}; out->clsidService = ServiceId; out->guidItem = GUID_LBI_INPUTMODE;
    out->dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_TEXTCOLORICON | TF_LBI_STYLE_SHOWNINTRAY;
    wcscpy_s(out->szDescription,L"善解中英切換"); return S_OK;
}
HRESULT ModeButton::GetStatus(DWORD* out) { *out = shown_ ? 0 : TF_LBI_STATUS_HIDDEN; return S_OK; }
HRESULT ModeButton::Show(BOOL show) { shown_ = show != FALSE; refresh(); return S_OK; }
HRESULT ModeButton::GetTooltipString(BSTR* out) { *out = SysAllocString(L"善解：點一下切換中英；右鍵開啟設定"); return *out ? S_OK : E_OUTOFMEMORY; }
HRESULT ModeButton::OnClick(TfLBIClick click,POINT,const RECT*) { if (click == TF_LBI_CLK_RIGHT) { settings_(); return S_OK; } return toggle_(); }
HRESULT ModeButton::InitMenu(ITfMenu* menu) { return menu->AddMenuItem(1,0,nullptr,nullptr,L"善解設定",4,nullptr); }
HRESULT ModeButton::OnMenuSelect(UINT item) { if (item == 1) settings_(); return S_OK; }
HRESULT ModeButton::GetText(BSTR* out) { *out = SysAllocString(chinese_() ? L"中" : L"英"); return *out ? S_OK : E_OUTOFMEMORY; }
HRESULT ModeButton::GetIcon(HICON* out) {
    int width = GetSystemMetrics(SM_CXSMICON), height = GetSystemMetrics(SM_CYSMICON);
    HDC dc = CreateCompatibleDC(nullptr);
    // TEXTCOLORICON requires a monochrome AND/XOR mask so TSF can apply its theme text color.
    HBITMAP mask = CreateBitmap(width,height*2,1,1,nullptr);
    auto old = SelectObject(dc,mask);
    PatBlt(dc,0,0,width,height,WHITENESS);
    PatBlt(dc,0,height,width,height,BLACKNESS);
    HFONT font = CreateFontW(-height+2,0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,0,0,NONANTIALIASED_QUALITY,0,L"Microsoft JhengHei UI");
    auto oldFont = SelectObject(dc,font); SetBkMode(dc,TRANSPARENT); SetTextColor(dc,RGB(0,0,0)); RECT rect{0,0,width,height};
    DrawTextW(dc,chinese_() ? L"中" : L"英",1,&rect,DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    GdiFlush();
    SelectObject(dc,oldFont); DeleteObject(font); SelectObject(dc,old);
    ICONINFO icon{TRUE,0,0,mask,nullptr}; *out = CreateIconIndirect(&icon);
    DeleteObject(mask); DeleteDC(dc); return *out ? S_OK : E_FAIL;
}
HRESULT ModeButton::AdviseSink(REFIID id,IUnknown* sink,DWORD* cookie) {
    if (id != IID_ITfLangBarItemSink) return E_NOINTERFACE;
    HRESULT hr = sink->QueryInterface(IID_PPV_ARGS(&sink_)); if (SUCCEEDED(hr)) *cookie = 1; return hr;
}
HRESULT ModeButton::UnadviseSink(DWORD cookie) { if (cookie != 1) return E_INVALIDARG; sink_.Reset(); return S_OK; }
void ModeButton::refresh() { if (sink_) sink_->OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_STATUS); }

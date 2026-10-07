#include "mode_indicator.h"
#include "identity.h"
#include <algorithm>
ModeIndicator::~ModeIndicator() { if (window_) DestroyWindow(window_); if (icon_) DestroyIcon(icon_); }
void ModeIndicator::hide() { if (window_) { KillTimer(window_,1); ShowWindow(window_,SW_HIDE); } }
void ModeIndicator::show(const RECT& caret,UINT dpi,bool chinese) {
    if (GetEnvironmentVariableW(L"SHANJIE_HEADLESS",nullptr,0)) return;
    if (!window_) {
        WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = module; wc.lpszClassName = L"ShanjieModeIndicator";
        RegisterClassW(&wc);
        window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,wc.lpszClassName,L"",WS_POPUP,0,0,0,0,nullptr,nullptr,module,this);
    }
    int size = MulDiv(40,dpi,96), inset = MulDiv(6,dpi,96);
    if (icon_) DestroyIcon(icon_);
    icon_ = (HICON)LoadImageW(module,MAKEINTRESOURCEW(chinese ? 101 : 102),IMAGE_ICON,size-inset*2,size-inset*2,0);
    MONITORINFO monitor{sizeof(monitor)}; GetMonitorInfoW(MonitorFromRect(&caret,MONITOR_DEFAULTTONEAREST),&monitor);
    int x = std::clamp((int)caret.left,(int)monitor.rcWork.left,(int)monitor.rcWork.right-size);
    int y = caret.bottom + inset;
    if (y + size > monitor.rcWork.bottom) y = caret.top-size-inset;
    if (GetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",nullptr,0)) x = y = -12000;
    SetWindowRgn(window_,CreateRoundRectRgn(0,0,size+1,size+1,inset*2,inset*2),FALSE);
    SetWindowPos(window_,HWND_TOPMOST,x,y,size,size,SWP_NOACTIVATE | SWP_SHOWWINDOW);
    InvalidateRect(window_,nullptr,FALSE); SetTimer(window_,1,1400,nullptr);
}
LRESULT CALLBACK ModeIndicator::procedure(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    auto self = reinterpret_cast<ModeIndicator*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (message == WM_NCCREATE) { self = (ModeIndicator*)((CREATESTRUCTW*)lp)->lpCreateParams; SetWindowLongPtrW(window,GWLP_USERDATA,(LONG_PTR)self); }
    if (message == WM_MOUSEACTIVATE) return MA_NOACTIVATE;
    if (self && message == WM_TIMER) { self->hide(); return 0; }
    if (self && (message == WM_PAINT || message == WM_PRINTCLIENT)) {
        PAINTSTRUCT paint{}; auto dc = message == WM_PAINT ? BeginPaint(window,&paint) : (HDC)wp; RECT bounds{}; GetClientRect(window,&bounds);
        FillRect(dc,&bounds,GetSysColorBrush(COLOR_WINDOW));
        int inset = bounds.right*6/40;
        DrawIconEx(dc,inset,inset,self->icon_,bounds.right-inset*2,bounds.bottom-inset*2,0,nullptr,DI_NORMAL);
        if (message == WM_PAINT) EndPaint(window,&paint); return 0;
    }
    return DefWindowProcW(window,message,wp,lp);
}

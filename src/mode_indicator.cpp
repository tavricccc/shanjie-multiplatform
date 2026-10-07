#include "mode_indicator.h"
#include "identity.h"
#include <gdiplus.h>
#include <algorithm>
// Geometry comes from assets/mode/*.svg, rendered directly at the destination pixel size.
static HBITMAP renderIcon(int size,bool chinese) {
    BITMAPINFO info{}; info.bmiHeader = {sizeof(BITMAPINFOHEADER),size,-size,1,32,BI_RGB};
    void* pixels = nullptr; auto bitmap = CreateDIBSection(nullptr,&info,DIB_RGB_COLORS,&pixels,nullptr,0);
    if (!bitmap) return nullptr;
    Gdiplus::Bitmap image(size,size,size*4,PixelFormat32bppPARGB,(BYTE*)pixels);
    Gdiplus::Graphics graphics(&image); graphics.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);
    graphics.Clear(Gdiplus::Color(0,0,0,0)); graphics.ScaleTransform(size/24.0f,size/24.0f);
    Gdiplus::GraphicsPath background;
    background.AddArc(1,1,8,8,180,90); background.AddArc(15,1,8,8,270,90);
    background.AddArc(15,15,8,8,0,90); background.AddArc(1,15,8,8,90,90); background.CloseFigure();
    Gdiplus::SolidBrush fill(chinese ? Gdiplus::Color(255,0x24,0x61,0x78) : Gdiplus::Color(255,0x47,0x55,0x69));
    graphics.FillPath(&fill,&background);
    Gdiplus::Pen stroke(Gdiplus::Color(255,255,255,255),2.2f);
    stroke.SetStartCap(Gdiplus::LineCapRound); stroke.SetEndCap(Gdiplus::LineCapRound); stroke.SetLineJoin(Gdiplus::LineJoinRound);
    Gdiplus::GraphicsPath glyph;
    if (chinese) {
        glyph.AddLine(12.0f,4.5f,12.0f,19.5f); glyph.StartFigure(); glyph.AddRectangle(Gdiplus::RectF(5.5f,8,13,8));
    } else {
        glyph.AddLine(6.5f,18.0f,12.0f,5.5f); glyph.AddLine(12.0f,5.5f,17.5f,18.0f);
        glyph.StartFigure(); glyph.AddLine(8.25f,14.0f,15.75f,14.0f);
    }
    graphics.DrawPath(&stroke,&glyph); graphics.Flush(Gdiplus::FlushIntentionSync); return bitmap;
}
ModeIndicator::~ModeIndicator() {
    if (window_) DestroyWindow(window_); if (bitmap_) DeleteObject(bitmap_);
    if (graphicsToken_) Gdiplus::GdiplusShutdown(graphicsToken_);
}
void ModeIndicator::hide() { if (window_) { KillTimer(window_,1); ShowWindow(window_,SW_HIDE); } }
void ModeIndicator::show(const RECT& caret,UINT dpi,bool chinese) {
    if (GetEnvironmentVariableW(L"SHANJIE_HEADLESS",nullptr,0)) return;
    if (!graphicsToken_) { Gdiplus::GdiplusStartupInput input; if (Gdiplus::GdiplusStartup(&graphicsToken_,&input,nullptr) != Gdiplus::Ok) return; }
    auto awareness = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    if (!window_) {
        WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = module; wc.lpszClassName = L"ShanjieModeIndicator"; RegisterClassW(&wc);
        window_ = CreateWindowExW(WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,wc.lpszClassName,L"",WS_POPUP,0,0,0,0,nullptr,nullptr,module,this);
    }
    SetWindowPos(window_,nullptr,caret.left,caret.bottom,0,0,SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER);
    if (!GetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",nullptr,0)) dpi = GetDpiForWindow(window_);
    int size = MulDiv(28,dpi,96), gap = MulDiv(6,dpi,96);
    if (bitmap_) DeleteObject(bitmap_); bitmap_ = renderIcon(size,chinese);
    MONITORINFO monitor{sizeof(monitor)}; GetMonitorInfoW(MonitorFromRect(&caret,MONITOR_DEFAULTTONEAREST),&monitor);
    POINT position{std::clamp((int)caret.left,(int)monitor.rcWork.left,(int)monitor.rcWork.right-size),caret.bottom+gap};
    if (position.y+size > monitor.rcWork.bottom) position.y = caret.top-size-gap;
    if (GetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",nullptr,0)) position = {-12000,-12000};
    auto dc = CreateCompatibleDC(nullptr); auto old = SelectObject(dc,bitmap_);
    POINT source{}; SIZE dimensions{size,size}; BLENDFUNCTION blend{AC_SRC_OVER,0,255,AC_SRC_ALPHA};
    UpdateLayeredWindow(window_,nullptr,&position,&dimensions,dc,&source,0,&blend,ULW_ALPHA);
    SelectObject(dc,old); DeleteDC(dc); ShowWindow(window_,SW_SHOWNOACTIVATE); SetTimer(window_,1,1400,nullptr);
    SetThreadDpiAwarenessContext(awareness);
}
LRESULT CALLBACK ModeIndicator::procedure(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    auto self = (ModeIndicator*)GetWindowLongPtrW(window,GWLP_USERDATA);
    if (message == WM_NCCREATE) { self = (ModeIndicator*)((CREATESTRUCTW*)lp)->lpCreateParams; SetWindowLongPtrW(window,GWLP_USERDATA,(LONG_PTR)self); }
    if (message == WM_MOUSEACTIVATE) return MA_NOACTIVATE;
    if (self && message == WM_TIMER) { self->hide(); return 0; }
    if (self && message == WM_PRINTCLIENT && self->bitmap_) {
        auto dc = CreateCompatibleDC(nullptr); auto old = SelectObject(dc,self->bitmap_); BITMAP bitmap{}; GetObjectW(self->bitmap_,sizeof(bitmap),&bitmap);
        BitBlt((HDC)wp,0,0,bitmap.bmWidth,bitmap.bmHeight,dc,0,0,SRCCOPY);
        SelectObject(dc,old); DeleteDC(dc); return 0;
    }
    return DefWindowProcW(window,message,wp,lp);
}

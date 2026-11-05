#include "identity.h"
#include "mode_indicator.h"
#include <cassert>
#include <cstdio>
#include <algorithm>
#include <vector>
#include "native_capture.h"
HMODULE module = nullptr;
long objects = 0;
int wmain(int argc,wchar_t** argv) {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    assert(argc == 2 || argc == 3); module = LoadLibraryW(argv[1]); assert(module);
    auto loadIcon = reinterpret_cast<intptr_t(__cdecl*)(bool)>(GetProcAddress(module,"sj_mode_icon")); assert(loadIcon);
    for (bool mode : {true,false}) for (int iconSize : {0,16,20,24}) {
        HICON icon = iconSize ? (HICON)LoadImageW(module,MAKEINTRESOURCEW(mode?101:102),IMAGE_ICON,iconSize,iconSize,0) : (HICON)loadIcon(mode); assert(icon);
        ICONINFO info{}; assert(GetIconInfo(icon,&info));
        assert(info.hbmColor); BITMAP bitmap{}; assert(GetObjectW(info.hbmColor,sizeof(bitmap),&bitmap));
        assert(bitmap.bmBitsPixel == 32 && bitmap.bmHeight == (iconSize?iconSize:GetSystemMetrics(SM_CYSMICON)));
        BITMAPINFO dib{}; dib.bmiHeader.biSize=sizeof(BITMAPINFOHEADER);
        dib.bmiHeader.biWidth=bitmap.bmWidth; dib.bmiHeader.biHeight=-bitmap.bmHeight;
        dib.bmiHeader.biPlanes=1; dib.bmiHeader.biBitCount=32; dib.bmiHeader.biCompression=BI_RGB;
        std::vector<DWORD> pixels(bitmap.bmWidth*bitmap.bmHeight); auto dc=CreateCompatibleDC(nullptr);
        assert(GetDIBits(dc,info.hbmColor,0,bitmap.bmHeight,pixels.data(),&dib,DIB_RGB_COLORS)); DeleteDC(dc);
        int first=bitmap.bmHeight,last=-1;
        for(int y=0;y<bitmap.bmHeight;y++)for(int x=0;x<bitmap.bmWidth;x++){
            auto p=pixels[y*bitmap.bmWidth+x];
            if((p>>24)>192&&(p&255)>192&&((p>>8)&255)>192&&((p>>16)&255)>192){first=std::min(first,y);last=std::max(last,y);}
        }
        if(bitmap.bmHeight<=24)assert((last-first+1)*100>=bitmap.bmHeight*70);
        std::printf("Tray glyph %s: %d/%ld pixels high\n",mode?"Chinese":"English",last-first+1,bitmap.bmHeight);
        DeleteObject(info.hbmColor); DeleteObject(info.hbmMask); DestroyIcon(icon);
    }
    SetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",L"1");
    {
        ModeIndicator indicator; RECT caret{100,100,101,120}; auto foreground = GetForegroundWindow();
        indicator.show(caret,96,true);
        auto window = FindWindowW(L"ShanjieRustModeIndicator",nullptr); assert(window && IsWindowVisible(window));
        RECT bounds{}; GetWindowRect(window,&bounds); assert(bounds.right < -10000 && bounds.bottom < -10000);
        assert(GetForegroundWindow() == foreground && SendMessageW(window,WM_MOUSEACTIVATE,0,0) == MA_NOACTIVATE);
        assert(GetWindowLongPtrW(window,GWL_EXSTYLE) & WS_EX_LAYERED);
        if (argc == 3) captureNative(window,std::filesystem::path(argv[2])/L"mode-chinese.bmp",true);
        indicator.hide(); assert(!IsWindowVisible(window)); indicator.show(caret,144,false);
        if (argc == 3) captureNative(window,std::filesystem::path(argv[2])/L"mode-english-144.bmp",true);
        GetWindowRect(window,&bounds); assert(bounds.right-bounds.left == 42);
        if (argc == 3) for (auto dpi : {96u,120u,144u,192u}) for (auto chineseMode : {true,false}) {
            indicator.show(caret,dpi,chineseMode);
            captureNative(window,std::filesystem::path(argv[2])/((chineseMode ? L"chinese-" : L"english-")+std::to_wstring(dpi)+L".bmp"),true);
        }
        auto deadline = GetTickCount64()+2500;
        while (IsWindowVisible(window) && GetTickCount64() < deadline) {
            MSG message; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&message); DispatchMessageW(&message); }
            MsgWaitForMultipleObjects(0,nullptr,FALSE,10,QS_ALLINPUT);
        }
        assert(!IsWindowVisible(window));
    }
    FreeLibrary(module); std::puts("PASS: full-color mode icons, DPI, no activation and timed hide");
}

#pragma once
#include <windows.h>
#include <filesystem>
#include <cassert>
inline void captureNative(HWND window,const std::filesystem::path& path) {
    RECT bounds{}; GetClientRect(window,&bounds); int width = bounds.right,height = bounds.bottom;
    BITMAPINFO bitmap{}; bitmap.bmiHeader = {sizeof(BITMAPINFOHEADER),width,-height,1,32,BI_RGB};
    void* pixels = nullptr; auto dc = CreateCompatibleDC(nullptr);
    auto image = CreateDIBSection(dc,&bitmap,DIB_RGB_COLORS,&pixels,nullptr,0); auto old = SelectObject(dc,image);
    SendMessageW(window,WM_PRINTCLIENT,(WPARAM)dc,PRF_CLIENT); GdiFlush();
    for (int i=0;i<width*height;++i) ((DWORD*)pixels)[i] |= 0xff000000;
    BITMAPFILEHEADER header{}; header.bfType = 0x4d42; header.bfOffBits = sizeof(header)+sizeof(BITMAPINFOHEADER); header.bfSize = header.bfOffBits+width*height*4;
    auto file = CreateFileW(path.c_str(),GENERIC_WRITE,0,nullptr,CREATE_ALWAYS,0,nullptr); assert(file != INVALID_HANDLE_VALUE); DWORD written;
    WriteFile(file,&header,sizeof(header),&written,nullptr); WriteFile(file,&bitmap.bmiHeader,sizeof(BITMAPINFOHEADER),&written,nullptr); WriteFile(file,pixels,width*height*4,&written,nullptr); CloseHandle(file);
    SelectObject(dc,old); DeleteObject(image); DeleteDC(dc);
}

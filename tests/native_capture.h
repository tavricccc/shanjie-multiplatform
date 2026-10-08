#pragma once
#include <windows.h>
#include <filesystem>
#include <cassert>
inline void captureNative(HWND window,const std::filesystem::path& path,bool preserveAlpha = true) {
    RECT bounds{}; GetClientRect(window,&bounds); int width = bounds.right,height = bounds.bottom;
    BITMAPV4HEADER bitmap{}; bitmap.bV4Size=sizeof(bitmap);bitmap.bV4Width=width;bitmap.bV4Height=-height;bitmap.bV4Planes=1;bitmap.bV4BitCount=32;bitmap.bV4V4Compression=BI_BITFIELDS;
    bitmap.bV4RedMask=0x00ff0000;bitmap.bV4GreenMask=0x0000ff00;bitmap.bV4BlueMask=0x000000ff;bitmap.bV4AlphaMask=0xff000000;bitmap.bV4CSType=LCS_sRGB;
    void* pixels = nullptr; auto dc = CreateCompatibleDC(nullptr);
    auto image = CreateDIBSection(dc,(BITMAPINFO*)&bitmap,DIB_RGB_COLORS,&pixels,nullptr,0); auto old = SelectObject(dc,image);
    SendMessageW(window,WM_PRINTCLIENT,(WPARAM)dc,PRF_CLIENT); GdiFlush();
    if (preserveAlpha) assert(((DWORD*)pixels)[0] == 0);
    else for (int i=0;i<width*height;++i) ((DWORD*)pixels)[i] |= 0xff000000;
    BITMAPFILEHEADER header{}; header.bfType = 0x4d42; header.bfOffBits = sizeof(header)+sizeof(bitmap); header.bfSize = header.bfOffBits+width*height*4;
    auto file = CreateFileW(path.c_str(),GENERIC_WRITE,0,nullptr,CREATE_ALWAYS,0,nullptr); assert(file != INVALID_HANDLE_VALUE); DWORD written;
    WriteFile(file,&header,sizeof(header),&written,nullptr); WriteFile(file,&bitmap,sizeof(bitmap),&written,nullptr); WriteFile(file,pixels,width*height*4,&written,nullptr); CloseHandle(file);
    SelectObject(dc,old); DeleteObject(image); DeleteDC(dc);
}

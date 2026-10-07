#pragma once
#include <windows.h>
class ModeIndicator {
    HWND window_ = nullptr;
    HBITMAP bitmap_ = nullptr;
    ULONG_PTR graphicsToken_ = 0;
    static LRESULT CALLBACK procedure(HWND,UINT,WPARAM,LPARAM);
public:
    ~ModeIndicator();
    void show(const RECT&,UINT,bool);
    void hide();
};

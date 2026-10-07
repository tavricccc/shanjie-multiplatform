#pragma once
#include <windows.h>
class ModeIndicator {
    HWND window_ = nullptr;
    HICON icon_ = nullptr;
    static LRESULT CALLBACK procedure(HWND,UINT,WPARAM,LPARAM);
public:
    ~ModeIndicator();
    void show(const RECT&,UINT,bool);
    void hide();
};

#pragma once
#include <windows.h>
class ModeIndicator {
    void* handle_;
public:
    ModeIndicator(); ~ModeIndicator();void show(const RECT&,UINT,bool);void hide();
};

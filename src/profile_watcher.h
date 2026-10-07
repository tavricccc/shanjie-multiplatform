#pragma once
#include <windows.h>
#include <functional>
// A hidden top-level window receives profile-save broadcasts on the TSF thread.
class ProfileWatcher {
    HWND window_ = nullptr;
    std::function<void()> changed_;
    static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM);
public:
    bool start(std::function<void()> changed);
    void stop();
    ~ProfileWatcher() { stop(); }
};

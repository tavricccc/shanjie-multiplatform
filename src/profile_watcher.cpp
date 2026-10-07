#include "profile_watcher.h"
#include "identity.h"
namespace { UINT profileMessage() { static auto value = RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"); return value; } }
bool ProfileWatcher::start(std::function<void()> changed) {
    changed_ = std::move(changed);
    WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = module; wc.lpszClassName = L"ShanjieProfileWatcher";
    RegisterClassW(&wc);
    window_ = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,wc.lpszClassName,L"",WS_POPUP,0,0,0,0,nullptr,nullptr,module,this);
    return window_ != nullptr;
}
void ProfileWatcher::stop() { if (window_) DestroyWindow(window_); window_ = nullptr; changed_ = {}; }
LRESULT ProfileWatcher::procedure(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    auto self = reinterpret_cast<ProfileWatcher*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (message == WM_NCCREATE) { self = static_cast<ProfileWatcher*>(reinterpret_cast<CREATESTRUCTW*>(lp)->lpCreateParams); SetWindowLongPtrW(window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(self)); }
    if (self && message == profileMessage()) { self->changed_(); return 0; }
    return DefWindowProcW(window,message,wp,lp);
}

#pragma once
#include <functional>
class ProfileWatcher {
    void* handle_ = nullptr; std::function<void()> changed_;
    static void changed(void* context) { static_cast<ProfileWatcher*>(context)->changed_(); }
public:
    bool start(std::function<void()>); void stop(); ~ProfileWatcher(){stop();}
};

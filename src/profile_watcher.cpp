#include "profile_watcher.h"
extern "C" void* sj_watcher_new(void(*)(void*),void*);
extern "C" void sj_watcher_free(void*);
bool ProfileWatcher::start(std::function<void()> changed){changed_=std::move(changed);handle_=sj_watcher_new(ProfileWatcher::changed,this);return handle_!=nullptr;}
void ProfileWatcher::stop(){sj_watcher_free(handle_);handle_=nullptr;changed_={};}

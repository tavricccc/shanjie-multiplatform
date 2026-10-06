#pragma once
#include <windows.h>
#include <filesystem>
#include <string>
#include <vector>
#include "shanjie.h"

namespace shanjie {
struct Snapshot {
    bool handled = false;
    std::wstring commit, preedit;
    uint32_t cursor = 0, columns = 0, first = 0, total = 0;
    int selected = -1;
    std::vector<std::wstring> candidates;
};
std::wstring wide(const char* text);
std::string utf8(const std::wstring& text);
class Engine {
    ShanjieEngine* handle_ = nullptr;
    Snapshot copy(ShanjieOutput* output);
public:
    ~Engine();
    bool open(const std::filesystem::path& data, unsigned layout, unsigned ranking = 0, bool rightAnchor = false);
    bool key(ShanjieKey key, Snapshot& output);
    bool pick(unsigned index, Snapshot& output);
    bool reset(bool discard, Snapshot& output);
    void context(const std::wstring& text);
};
bool translate(WPARAM key, LPARAM flags, ShanjieKey& result);
}

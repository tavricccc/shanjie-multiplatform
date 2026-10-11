#pragma once
#include <windows.h>
#include <filesystem>
#include <string>
#include <vector>
#include "shanjie.h"

namespace shanjie {
struct Snapshot {
    bool handled = false;
    bool associated = false;
    bool prediction = false;
    std::wstring commit, preedit;
    uint32_t cursor = 0, columns = 0, first = 0, total = 0;
    int selected = -1;
    std::vector<std::wstring> candidates;
};
std::wstring wide(const char* text);
std::string utf8(const std::wstring& text);
class Engine {
    void* handle_ = nullptr;
    bool action(unsigned,Snapshot&,ShanjieKey = {},unsigned = 0,const char* = "");
public:
    void* native() { return handle_; }
    ~Engine();
    bool open(const std::filesystem::path& data, unsigned layout, unsigned ranking = 0, bool rightAnchor = false, bool learning = false, bool big5 = false, const std::wstring& dictionary = L"dictionary.tsv", unsigned model = 5, bool prediction = false);
    bool key(ShanjieKey key, Snapshot& output);
    bool pick(unsigned index, Snapshot& output);
    bool reset(bool discard, Snapshot& output);
    bool associate(const std::wstring& prefix, Snapshot& output);
    bool commitReading(Snapshot&);
    bool prediction(bool enabled, Snapshot&);
    bool demote(bool enabled, Snapshot&);
    bool inputOptions(bool shift, bool plain, bool tones);
    bool punctuation(const std::wstring&, Snapshot&);
    bool cycleCandidate(Snapshot&);
    bool smartInput(unsigned flags, Snapshot& out) { return action(9,out,{},flags); }
    void context(const std::wstring& text);
};
bool translate(WPARAM key, LPARAM flags, ShanjieKey& result);
}

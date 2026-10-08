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
    std::wstring commit, preedit, reading;
    uint32_t cursor = 0, columns = 0, first = 0, total = 0;
    int selected = -1;
    std::vector<std::wstring> candidates;
};
std::wstring wide(const char* text);
std::string utf8(const std::wstring& text);
class Engine {
    ShanjieEngine* handle_ = nullptr;
    HANDLE learningMutex_ = nullptr;
    std::filesystem::path learningPath_;
    std::string learningEpoch_;
    bool readingCandidate_ = false;
    bool beginLearning();
    void endLearning();
    Snapshot copy(ShanjieOutput* output);
public:
    ~Engine();
    bool open(const std::filesystem::path& data, unsigned layout, unsigned ranking = 0, bool rightAnchor = false, bool learning = false, bool big5 = false, const std::wstring& dictionary = L"dictionary.tsv", unsigned model = 1);
    bool key(ShanjieKey key, Snapshot& output);
    bool pick(unsigned index, Snapshot& output);
    bool reset(bool discard, Snapshot& output);
    bool associate(const std::wstring& prefix, Snapshot& output);
    void setReadingCandidate(bool enabled) { readingCandidate_ = enabled; }
    bool commitReading(Snapshot&);
    bool prediction(bool enabled, Snapshot&);
    bool demote(bool enabled, Snapshot&);
    void context(const std::wstring& text);
};
bool translate(WPARAM key, LPARAM flags, ShanjieKey& result);
}

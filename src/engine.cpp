#include "engine.h"
#include "windows_core.h"
#include <memory>
#include <algorithm>
#include <fstream>
namespace shanjie {
std::wstring wide(const char* text) {
    int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, nullptr, 0);
    if (size <= 1) return {};
    std::wstring value(size, L'\0');
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, value.data(), size);
    value.resize(size - 1);
    return value;
}
std::string utf8(const std::wstring& text) {
    int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), (int)text.size(), nullptr, 0, nullptr, nullptr);
    std::string value(size, '\0');
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), (int)text.size(), value.data(), size, nullptr, nullptr);
    return value;
}
Engine::~Engine() { shanjie_engine_free(handle_); if (learningMutex_) CloseHandle(learningMutex_); }
bool Engine::beginLearning() {
    if (!learningMutex_) return true;
    DWORD result = WaitForSingleObject(learningMutex_,250);
    if (result != WAIT_OBJECT_0 && result != WAIT_ABANDONED) return false;
    std::ifstream epochFile(learningPath_ / L"learning.epoch"); std::string epoch; epochFile >> epoch;
    if (epoch != learningEpoch_) { shanjie_engine_set_learning(handle_,0); shanjie_engine_set_learning(handle_,1); learningEpoch_ = epoch; }
    if (shanjie_engine_learning_open(handle_,utf8(learningPath_.wstring()).c_str()) == 0) return true;
    ReleaseMutex(learningMutex_); return false;
}
void Engine::endLearning() { if (learningMutex_) ReleaseMutex(learningMutex_); }
bool Engine::open(const std::filesystem::path& data, unsigned layout, unsigned ranking, bool rightAnchor, bool learning, bool big5, const std::wstring& dictionaryName, unsigned model) {
    if (model < 1 || model > 4) return false;
    shanjie_engine_free(handle_); handle_ = nullptr;
    wchar_t localDictionary[32768]; auto localLength = GetEnvironmentVariableW(L"LOCALAPPDATA",localDictionary,32768);
    auto personal = localLength && localLength < 32768 ? std::filesystem::path(localDictionary) / L"Shanjie" / dictionaryName : std::filesystem::path{};
    if (dictionaryName != L"dictionary.tsv" && !std::filesystem::exists(personal)) return false;
    std::string dictionary = !personal.empty() && std::filesystem::exists(personal) && std::filesystem::file_size(personal) ? utf8(personal.wstring()) : std::string{};
    if (shanjie_windows_engine_new(utf8(data.wstring()).c_str(),dictionary.empty() ? nullptr : dictionary.c_str(),layout,&handle_) != 0) return false;
    // Windows starts without passive prediction, so its index is not allocated.
    ShanjieOutput* output = nullptr;
    auto result = shanjie_engine_set_prediction(handle_,0,&output);
    if (output) shanjie_output_free(output);
    if (result != 0) return false;
    auto modelPath = data / L"models" / (L"model-v" + std::to_wstring(model)) / L"bigram.winmap";
    if (shanjie_engine_load_lm(handle_, utf8(modelPath.wstring()).c_str()) != 0) {
        shanjie_engine_free(handle_); handle_ = nullptr; return false;
    }
    output = nullptr;
    result = shanjie_engine_set_profile(handle_,ranking,&output);
    if (output) shanjie_output_free(output);
    if (result != 0 || shanjie_windows_set_candidate_anchor(handle_,rightAnchor ? 1 : 0) != 0) return false;
    if (shanjie_windows_set_candidate_charset(handle_,big5 ? 1 : 0) != 0) return false;
    if (learningMutex_) { CloseHandle(learningMutex_); learningMutex_ = nullptr; }
    if (learning) {
        wchar_t local[32768]; DWORD length = GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768);
        if (!length || length >= 32768) return false;
        learningPath_ = std::filesystem::path(local) / L"Shanjie";
        auto name = learningPath_.wstring(); CharUpperBuffW(name.data(),(DWORD)name.size());
        std::replace(name.begin(),name.end(),L'\\',L'_');
        learningMutex_ = CreateMutexW(nullptr,FALSE,(L"Local\\ShanjieLearning-" + name).c_str());
        if (!learningMutex_ || !beginLearning()) return false;
        result = shanjie_engine_set_learning(handle_,1); endLearning();
    }
    return result == 0;
}
Snapshot Engine::copy(ShanjieOutput* raw) {
    std::unique_ptr<ShanjieOutput, decltype(&shanjie_output_free)> owner(raw, shanjie_output_free);
    Snapshot out;
    out.handled = raw->handled != 0;
    out.commit = wide(raw->commit); out.preedit = wide(raw->preedit);
    out.cursor = raw->cursor_utf16; out.selected = raw->candidate_selected;
    out.prediction = shanjie_windows_is_prediction_row(handle_) != 0;
    out.columns = raw->candidate_columns; out.first = raw->candidate_first; out.total = raw->candidate_total;
    for (unsigned i = 0; i < raw->candidate_count; ++i) out.candidates.push_back(wide(raw->candidates[i]));
    if (readingCandidate_ && raw->candidate_count) {
        ShanjieOutput* reading = nullptr;
        if (shanjie_windows_reading(handle_,0,&reading) == 0) { out.reading = wide(reading->preedit); shanjie_output_free(reading); }
    }
    return out;
}
bool Engine::key(ShanjieKey key, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || !beginLearning()) return false;
    auto result = shanjie_engine_key(handle_, key, &raw); endLearning();
    if (result != 0) return false;
    output = copy(raw); return true;
}
bool Engine::pick(unsigned index, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || !beginLearning()) return false;
    auto result = shanjie_engine_pick(handle_, index, &raw); endLearning();
    if (result != 0) return false;
    output = copy(raw); return true;
}
bool Engine::reset(bool discard, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || !beginLearning()) return false;
    auto result = shanjie_engine_reset(handle_, discard ? 1 : 0, &raw); endLearning();
    if (result != 0) return false;
    output = copy(raw); return true;
}
void Engine::context(const std::wstring& text) {
    shanjie_engine_set_left_context(handle_, utf8(text).c_str());
}
bool Engine::associate(const std::wstring& prefix,Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || shanjie_windows_associate(handle_,utf8(prefix).c_str(),&raw) != 0) return false;
    output = copy(raw); output.associated = true; return true;
}
bool Engine::commitReading(Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || shanjie_windows_reading(handle_,1,&raw) != 0) return false;
    output = copy(raw); return true;
}
bool Engine::prediction(bool enabled, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || shanjie_engine_set_prediction(handle_,enabled ? 1 : 0,&raw) != 0) return false;
    output = copy(raw); return true;
}
bool Engine::demote(bool enabled, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || shanjie_engine_set_demote(handle_,enabled ? 1 : 0,&raw) != 0) return false;
    output = copy(raw); return true;
}
bool translate(WPARAM vk, LPARAM flags, ShanjieKey& result) {
    result = {};
    if (GetKeyState(VK_SHIFT) & 0x8000) result.modifiers |= 1;
    if (GetKeyState(VK_CONTROL) & 0x8000) result.modifiers |= 2;
    if (GetKeyState(VK_MENU) & 0x8000) result.modifiers |= 4;
    if ((GetKeyState(VK_LWIN) | GetKeyState(VK_RWIN)) & 0x8000) result.modifiers |= 8;
    if (GetKeyState(VK_CAPITAL) & 1) result.modifiers |= 16;
    switch (vk) {
    case VK_SPACE: result.kind = 2; break; case VK_RETURN: result.kind = 3; break;
    case VK_BACK: result.kind = 4; break; case VK_DELETE: result.kind = 5; break;
    case VK_ESCAPE: result.kind = 6; break; case VK_LEFT: result.kind = 7; break;
    case VK_RIGHT: result.kind = 8; break; case VK_UP: result.kind = 9; break;
    case VK_DOWN: result.kind = 10; break; case VK_HOME: result.kind = 11; break;
    case VK_END: result.kind = 12; break; case VK_TAB: result.kind = 13; break;
    default: {
        BYTE state[256] = {}; WCHAR chars[8] = {};
        // The ABI expects the unshifted keycap; Shift is conveyed separately.
        int count = ToUnicodeEx((UINT)vk, (UINT)((flags >> 16) & 255), state, chars, 8, 4, GetKeyboardLayout(0));
        if (count != 1 || chars[0] < 32 || chars[0] > 126) return false;
        result.kind = 1; result.ch = chars[0];
    }}
    return true;
}
}

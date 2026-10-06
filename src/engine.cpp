#include "engine.h"
#include <memory>
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
Engine::~Engine() { shanjie_engine_free(handle_); }
bool Engine::open(const std::filesystem::path& data, unsigned layout) {
    shanjie_engine_free(handle_); handle_ = nullptr;
    if (shanjie_engine_new(utf8(data.wstring()).c_str(), layout, &handle_) != 0) return false;
    if (shanjie_engine_load_lm(handle_, utf8((data / L"bigram.winmap").wstring()).c_str()) != 0) {
        shanjie_engine_free(handle_); handle_ = nullptr; return false;
    }
    return true;
}
Snapshot Engine::copy(ShanjieOutput* raw) {
    std::unique_ptr<ShanjieOutput, decltype(&shanjie_output_free)> owner(raw, shanjie_output_free);
    Snapshot out;
    out.handled = raw->handled != 0;
    out.commit = wide(raw->commit); out.preedit = wide(raw->preedit);
    out.cursor = raw->cursor_utf16; out.selected = raw->candidate_selected;
    out.columns = raw->candidate_columns; out.first = raw->candidate_first; out.total = raw->candidate_total;
    for (unsigned i = 0; i < raw->candidate_count; ++i) out.candidates.push_back(wide(raw->candidates[i]));
    return out;
}
bool Engine::key(ShanjieKey key, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || shanjie_engine_key(handle_, key, &raw) != 0) return false;
    output = copy(raw); return true;
}
bool Engine::pick(unsigned index, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || shanjie_engine_pick(handle_, index, &raw) != 0) return false;
    output = copy(raw); return true;
}
bool Engine::reset(bool discard, Snapshot& output) {
    ShanjieOutput* raw = nullptr;
    if (!handle_ || shanjie_engine_reset(handle_, discard ? 1 : 0, &raw) != 0) return false;
    output = copy(raw); return true;
}
void Engine::context(const std::wstring& text) {
    shanjie_engine_set_left_context(handle_, utf8(text).c_str());
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

#include "key_policy.h"
#include <string_view>
namespace shanjie {
bool windowsPunctuation(const ShanjieKey& key) {
    return key.kind == 1 && (key.modifiers == 2 || key.modifiers == 3)
        && std::string_view(",./1;[]90`\\'=").find((char)key.ch) != std::string_view::npos
        && (std::string_view("190").find((char)key.ch) == std::string_view::npos || key.modifiers == 3);
}
std::wstring windowsPunctuationText(const ShanjieKey& key) {
    bool shift = key.modifiers & 1;
    switch (key.ch) {
    case ',': return shift ? L"〈" : L"，"; case '.': return shift ? L"〉" : L"。";
    case '/': return shift ? L"？" : L"／"; case '1': return L"！";
    case ';': return shift ? L"：" : L"；"; case '[': return shift ? L"『" : L"「";
    case ']': return shift ? L"』" : L"」"; case '9': return L"（"; case '0': return L"）";
    case '\\': return L"、"; case '\'': return shift ? L"＂" : L"＇";
    case '=': return shift ? L"＋" : L"＝"; case '`': return L"｀";
    default: return {};
    }
}
bool hasPendingReading(const Snapshot& state) {
    for (auto ch : state.preedit) if (ch >= 0x3105 && ch <= 0x312f) return true;
    return false;
}
ShanjieKey windowsKey(ShanjieKey key, const Snapshot& state, const Preferences& preferences) {
    if (state.prediction) {
        if (preferences.windowsKeys && state.selected >= 0 && key.modifiers == 0) {
            if (key.kind == 10) key.kind = 8;
            else if (key.kind == 9) key.kind = 7;
        }
        return key; // Tab enters/moves prediction; it must never expand a conversion grid.
    }
    if (!state.candidates.empty() && state.columns == 0 && key.modifiers == 0 && preferences.windowsKeys) {
        switch (key.kind) {
        case 10: key.kind = 8; break; // Windows advances with Down in both list orientations.
        case 9: key.kind = 7; break;  // Up: previous item.
        case 8: if (preferences.rightExpands) key.kind = 10; break;
        }
    } else if (!state.candidates.empty() && !state.columns && key.kind == 8 && key.modifiers == 0 && preferences.rightExpands) {
        key.kind = 10;
    }
    if (!state.candidates.empty() && key.kind == 13 && key.modifiers == 0 && preferences.tabExpands) {
        key.kind = 10;
    }
    return key;
}
}

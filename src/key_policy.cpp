#include "key_policy.h"
#include <string_view>
namespace shanjie {
bool windowsPunctuation(const ShanjieKey& key) {
    return key.kind == 1 && (key.modifiers == 2 || key.modifiers == 3)
        && std::string_view(",./1;[]90`\\'=").find((char)key.ch) != std::string_view::npos;
}
bool hasPendingReading(const Snapshot& state) {
    for (auto ch : state.preedit) if (ch >= 0x3105 && ch <= 0x312f) return true;
    return false;
}
ShanjieKey windowsKey(ShanjieKey key, const Snapshot& state, bool vertical) {
    if (windowsPunctuation(key)) { key.modifiers = 1; return key; }
    if (vertical && !state.candidates.empty() && state.columns == 0 && key.modifiers == 0) {
        switch (key.kind) {
        case 10: key.kind = 8; break; // Down: next item in vertical list.
        case 9: key.kind = 7; break;  // Up: previous item.
        case 8: case 13: key.kind = 10; break; // Right/Tab: expand grid.
        }
    } else if (!state.candidates.empty() && key.kind == 13 && key.modifiers == 0) {
        key.kind = 10;
    }
    return key;
}
}

#pragma once
#include "engine.h"
#include "preferences.h"
namespace shanjie {
// Microsoft Bopomofo uses a vertical list before expanding to a grid.
ShanjieKey windowsKey(ShanjieKey, const Snapshot&, const Preferences&);
std::wstring windowsPunctuationText(const ShanjieKey&);
bool hasPendingReading(const Snapshot&);
bool windowsPunctuation(const ShanjieKey&);
}

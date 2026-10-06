#pragma once
#include "engine.h"
namespace shanjie {
// Microsoft Bopomofo uses a vertical list before expanding to a grid.
ShanjieKey windowsKey(ShanjieKey, const Snapshot&, bool vertical);
bool hasPendingReading(const Snapshot&);
bool windowsPunctuation(const ShanjieKey&);
}

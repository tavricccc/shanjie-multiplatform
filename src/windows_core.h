#pragma once
#include "shanjie.h"
// Extension implemented by the Windows build overlay, not upstream shanjie.h.
extern "C" int32_t shanjie_windows_set_candidate_anchor(ShanjieEngine*, uint32_t right);

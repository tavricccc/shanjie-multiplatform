#pragma once
#include "shanjie.h"
extern "C" int32_t shanjie_windows_associate(ShanjieEngine*, const char*, ShanjieOutput**);
extern "C" int32_t shanjie_windows_reading(ShanjieEngine*, uint32_t, ShanjieOutput**);
extern "C" int32_t shanjie_windows_set_candidate_charset(ShanjieEngine*, uint32_t);
extern "C" int32_t shanjie_windows_engine_new(const char*, const char*, uint32_t, ShanjieEngine**);
#include "shanjie.h"
// Extension implemented by the Windows build overlay, not upstream shanjie.h.
extern "C" int32_t shanjie_windows_set_candidate_anchor(ShanjieEngine*, uint32_t right);

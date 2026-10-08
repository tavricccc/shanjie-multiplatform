#include "key_policy.h"
#include "rust_bridge.h"
namespace shanjie {
bool windowsPunctuation(const ShanjieKey& k){return sj_is_punctuation(k);} std::wstring windowsPunctuationText(const ShanjieKey& k){return wide(sj_punctuation(k));}
bool hasPendingReading(const Snapshot& s){FrameView f(s);return sj_pending(&f.raw);} ShanjieKey windowsKey(ShanjieKey k,const Snapshot& s,const Preferences& p){FrameView f(s);return sj_candidate_key(k,&f.raw,p.windowsKeys);}
}

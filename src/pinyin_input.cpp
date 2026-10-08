#include "pinyin_input.h"
#include "rust_bridge.h"
PinyinInput::PinyinInput():handle_(sj_pinyin_new()){} PinyinInput::~PinyinInput(){sj_pinyin_free(handle_);}
bool PinyinInput::load(const std::filesystem::path& data,bool taiwan){return sj_pinyin_load(handle_,shanjie::utf8(data.wstring()).c_str(),taiwan);}
bool PinyinInput::enabled()const{return sj_pinyin_enabled(handle_);} bool PinyinInput::active()const{return sj_pinyin_active(handle_);}
bool PinyinInput::handles(const ShanjieKey& k)const{return sj_pinyin_handles(handle_,k);} void PinyinInput::clear(){sj_pinyin_clear(handle_);}
bool PinyinInput::key(const ShanjieKey& k,shanjie::Engine& e,const shanjie::Snapshot& s,shanjie::Snapshot& out,bool ascii,bool space){FrameView f(s);auto frame=sj_pinyin_key(handle_,k,e.native(),&f.raw,ascii,space);if(!frame)return false;out=takeFrame(frame);return true;}

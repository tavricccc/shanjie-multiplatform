#include "special_input.h"
#include "rust_bridge.h"
namespace {
std::string options(const Preferences& p){return "[shortcuts]\nbackquote_symbols="+std::to_string(p.backquoteSymbols)+"\ncode_input="+std::to_string(p.codeInput)+"\nsymbol_keyboard="+std::to_string(p.symbolKeyboard)+"\n[candidates]\nspace_pages="+std::to_string(p.spacePages)+"\nwrap_pages="+std::to_string(p.wrapPages);}
}
SpecialInput::SpecialInput():handle_(sj_special_new()){} SpecialInput::~SpecialInput(){sj_special_free(handle_);}
bool SpecialInput::active()const{return sj_special_mode(handle_)!=0;} bool SpecialInput::textInput()const{auto m=sj_special_mode(handle_);return m!=0&&m!=4;}
bool SpecialInput::starts(const ShanjieKey& k,const Preferences& p)const{return sj_special_starts(k,options(p).c_str());}
shanjie::Snapshot SpecialInput::key(const ShanjieKey& k,const Preferences& p){return takeFrame(sj_special_key(handle_,k,options(p).c_str()));}
shanjie::Snapshot SpecialInput::pick(unsigned i){return takeFrame(sj_special_pick(handle_,i));} void SpecialInput::clear(){sj_special_clear(handle_);}

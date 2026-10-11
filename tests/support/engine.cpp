#include "engine.h"
#include "rust_bridge.h"
namespace shanjie {
std::wstring wide(const char* text) {int size=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,text,-1,nullptr,0);if(size<=1)return {};std::wstring value(size,L'\0');MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,text,-1,value.data(),size);value.resize(size-1);return value;}
std::string utf8(const std::wstring& text) {int size=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,text.data(),(int)text.size(),nullptr,0,nullptr,nullptr);std::string value(size,'\0');WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,text.data(),(int)text.size(),value.data(),size,nullptr,nullptr);return value;}
Engine::~Engine(){sj_engine_free(handle_);}
bool Engine::open(const std::filesystem::path& data,unsigned layout,unsigned ranking,bool right,bool learning,bool big5,const std::wstring& dictionary,unsigned model,bool prediction){if(model!=5)return false;sj_engine_free(handle_);handle_=sj_engine_new(utf8(data.wstring()).c_str(),layout,ranking,(right?1u:0u)|(learning?2u:0u)|(big5?4u:0u)|(prediction?8u:0u),utf8(dictionary).c_str());return handle_!=nullptr;}
bool Engine::action(unsigned op,Snapshot& out,ShanjieKey key,unsigned value,const char* text){auto f=sj_engine_action(handle_,op,key,value,text);if(!f)return false;out=takeFrame(f);return true;}
bool Engine::key(ShanjieKey key,Snapshot& out){return action(0,out,key);} bool Engine::pick(unsigned index,Snapshot& out){return action(1,out,{},index);}
bool Engine::reset(bool discard,Snapshot& out){return action(2,out,{},discard);} bool Engine::associate(const std::wstring& prefix,Snapshot& out){return action(3,out,{},0,utf8(prefix).c_str());}
bool Engine::commitReading(Snapshot& out){return action(4,out);} bool Engine::prediction(bool value,Snapshot& out){return action(5,out,{},value);}
bool Engine::demote(bool value,Snapshot& out){return action(6,out,{},value);} bool Engine::punctuation(const std::wstring& text,Snapshot& out){return action(7,out,{},0,utf8(text).c_str());}
bool Engine::cycleCandidate(Snapshot& out){return action(8,out);} void Engine::context(const std::wstring& text){sj_engine_context(handle_,utf8(text).c_str());}
bool Engine::inputOptions(bool shift,bool plain,bool tones){return sj_engine_options(handle_,(shift?1u:0u)|(plain?2u:0u)|(tones?4u:0u));}
bool translate(WPARAM key,LPARAM flags,ShanjieKey& out){return sj_translate(key,flags,&out);}
}

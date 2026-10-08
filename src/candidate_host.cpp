#include "candidate_host.h"
#include "rust_bridge.h"
CandidateHost::~CandidateHost(){sj_host_free(handle_);} bool CandidateHost::start(HWND callback){if(!handle_)handle_=sj_host_new((intptr_t)callback);return handle_!=nullptr;}
void CandidateHost::publish(const shanjie::Snapshot& state,const Preferences& p,const RECT& caret,UINT,bool visible){if(handle_){FrameView view(state);sj_host_publish(handle_,&view.raw,p.theme,p.fontSize,caret,visible);}}
void CandidateHost::hide(){if(handle_)sj_host_hide(handle_);} LONG CandidateHost::sequence()const{return handle_?sj_host_sequence(handle_):0;}
bool CandidateHost::shown()const{return handle_&&sj_host_shown(handle_);} HWND CandidateHost::window()const{return handle_?(HWND)sj_host_window(handle_):nullptr;}

#include "associated_candidates.h"
#include "rust_bridge.h"
AssociatedCandidates::AssociatedCandidates():handle_(sj_associated_new()){} AssociatedCandidates::~AssociatedCandidates(){sj_associated_free(handle_);}
bool AssociatedCandidates::active()const{return sj_associated_active(handle_);} void AssociatedCandidates::clear(){sj_associated_clear(handle_);}
void AssociatedCandidates::open(const shanjie::Snapshot& s,const shanjie::Snapshot& b){FrameView words(s),base(b);sj_associated_open(handle_,&words.raw,&base.raw);}
shanjie::Snapshot AssociatedCandidates::view()const{return takeFrame(sj_associated_view(handle_));}
bool AssociatedCandidates::handles(const ShanjieKey& k)const{return sj_associated_handles(handle_,k);}
shanjie::Snapshot AssociatedCandidates::key(const ShanjieKey& k){return takeFrame(sj_associated_key(handle_,k));}
shanjie::Snapshot AssociatedCandidates::pick(unsigned i){return takeFrame(sj_associated_pick(handle_,i));}

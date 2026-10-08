#pragma once
#include "engine.h"
class AssociatedCandidates {
    void* handle_;
public:
    AssociatedCandidates(); ~AssociatedCandidates(); bool active()const; void clear();
    void open(const shanjie::Snapshot&,const shanjie::Snapshot& = {});
    shanjie::Snapshot view()const; bool handles(const ShanjieKey&)const;
    shanjie::Snapshot key(const ShanjieKey&); shanjie::Snapshot pick(unsigned);
};

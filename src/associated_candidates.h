#pragma once
#include "engine.h"
class AssociatedCandidates {
    std::vector<std::wstring> words_;
    unsigned selected_ = 0;
    bool expanded_ = false;
public:
    bool active() const { return !words_.empty(); }
    void clear() { words_.clear(); selected_ = 0; expanded_ = false; }
    void open(const shanjie::Snapshot& out) { clear(); words_ = out.candidates; }
    shanjie::Snapshot view() const;
    bool handles(const ShanjieKey&) const;
    shanjie::Snapshot key(const ShanjieKey&);
    shanjie::Snapshot pick(unsigned);
};

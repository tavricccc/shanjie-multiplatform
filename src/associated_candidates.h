#pragma once
#include "engine.h"
class AssociatedCandidates {
    std::vector<std::wstring> words_;
    unsigned selected_ = 0;
    bool expanded_ = false;
    shanjie::Snapshot base_;
public:
    bool active() const { return !words_.empty(); }
    void clear() { words_.clear(); selected_ = 0; expanded_ = false; base_ = {}; }
    void open(const shanjie::Snapshot& out, const shanjie::Snapshot& base = {}) { clear(); words_ = out.candidates; base_ = base; base_.commit.clear(); base_.candidates.clear(); base_.prediction = false; }
    shanjie::Snapshot view() const;
    bool handles(const ShanjieKey&) const;
    shanjie::Snapshot key(const ShanjieKey&);
    shanjie::Snapshot pick(unsigned);
};

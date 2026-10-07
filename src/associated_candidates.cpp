#include "associated_candidates.h"
#include <algorithm>
shanjie::Snapshot AssociatedCandidates::view() const {
    shanjie::Snapshot out; out.handled = out.associated = true;
    if (!active()) return out;
    out.first = expanded_ ? 0 : selected_ / 9 * 9; out.total = (unsigned)words_.size();
    auto end = std::min<unsigned>(out.first + (expanded_ ? 81 : 9),out.total);
    out.candidates.assign(words_.begin() + out.first,words_.begin() + end);
    out.selected = (int)(selected_ - out.first); out.columns = expanded_ ? 9 : 0; return out;
}
bool AssociatedCandidates::handles(const ShanjieKey& key) const {
    if (!active()) return false;
    if (key.kind == 1) return key.modifiers == 1 && key.ch >= '1' && key.ch <= '9';
    return key.modifiers == 0 && (key.kind == 2 || key.kind == 3 || key.kind == 6 || (key.kind >= 7 && key.kind <= 13));
}
shanjie::Snapshot AssociatedCandidates::pick(unsigned index) {
    auto frame = view(); shanjie::Snapshot out; out.handled = true;
    if (index < frame.candidates.size()) out.commit = frame.candidates[index];
    clear(); return out;
}
shanjie::Snapshot AssociatedCandidates::key(const ShanjieKey& key) {
    if (key.kind == 1) return pick(selected_ / 9 * 9 - view().first + key.ch - '1');
    if (key.kind == 3) return pick(selected_ - view().first);
    if (key.kind == 6) { clear(); return view(); }
    if (key.kind == 2) { selected_ = selected_ / 9 * 9 + 9; if (selected_ >= words_.size()) selected_ = 0; }
    if (key.kind == 10) selected_ = std::min<unsigned>(selected_ + (expanded_ ? 9 : 1),(unsigned)words_.size()-1);
    if (key.kind == 9) selected_ = selected_ >= (expanded_ ? 9u : 1u) ? selected_ - (expanded_ ? 9 : 1) : 0;
    if (key.kind == 8 || key.kind == 13) { if (!expanded_) expanded_ = true; else selected_ = std::min<unsigned>(selected_ + 1,(unsigned)words_.size()-1); }
    if (key.kind == 7) selected_ = selected_ ? selected_-1 : 0;
    return view();
}

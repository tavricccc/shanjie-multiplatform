#pragma once
#include "engine.h"
#include "preferences.h"
class SpecialInput {
    enum class Mode { None, Prefix, Unicode, Big5, Symbols } mode_ = Mode::None;
    std::wstring digits_;
    unsigned selected_ = 0;
    bool expanded_ = false;
    shanjie::Snapshot view() const;
public:
    bool active() const { return mode_ != Mode::None; }
    bool starts(const ShanjieKey&, const Preferences&) const;
    shanjie::Snapshot key(const ShanjieKey&, const Preferences&);
    shanjie::Snapshot pick(unsigned index);
    void clear() { mode_ = Mode::None; digits_.clear(); selected_ = 0; expanded_ = false; }
};

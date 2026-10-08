#pragma once
#include "engine.h"
#include "preferences.h"
class SpecialInput {
    void* handle_;
public:
    SpecialInput(); ~SpecialInput(); bool active() const; bool textInput() const;
    bool starts(const ShanjieKey&,const Preferences&) const;
    shanjie::Snapshot key(const ShanjieKey&,const Preferences&); shanjie::Snapshot pick(unsigned); void clear();
};

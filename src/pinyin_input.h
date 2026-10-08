#pragma once
#include "engine.h"
class PinyinInput {
    void* handle_;
public:
    PinyinInput(); ~PinyinInput(); bool load(const std::filesystem::path&,bool); bool enabled()const; bool active()const;
    bool handles(const ShanjieKey&)const;
    bool key(const ShanjieKey&,shanjie::Engine&,const shanjie::Snapshot&,shanjie::Snapshot&,bool,bool = false); void clear();
};
